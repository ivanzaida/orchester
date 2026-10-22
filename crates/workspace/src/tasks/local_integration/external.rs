use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::local::{Checkout, ContainedResult, LocalError, LocalIntegrationStore, ResultEvidenceKind},
  revision::Revision,
  run::Run,
  store::{MergeStore, RunStore},
};
use orchester_exec::local_git::{LocalGit, LocalGitError};
use orchester_store::ProjectDb;

use super::{IntegrationError, LocalIntegration};

/// Positive ancestry evidence can block duplicate integration without inventing a merge event. A record of the run's
/// that holds nothing reads as complete once the evidence proves the whole work merged (`direct::settle_outside`).
pub fn external_integration(root: &Path, run: RunId) -> Result<Option<ContainedResult>, IntegrationError> {
  let scratch =
    orchester_exec::local_git::prepare_scratch(root, &[".orchester", "local-integration", &run.to_string()])?;
  let _store_owner = orchester_exec::local_git::owned::StoreOwnership::acquire(root)?;
  let mut db = ProjectDb::open(root)?;
  let main = LocalGit::new(root, &scratch)?;
  super::ensure_project_git(&main)?;
  let checkout = main.snapshot()?;
  let run = db
    .run(run)
    .map_err(|_| IntegrationError::Run)?
    .ok_or(IntegrationError::Run)?;
  let mut operation = db.local_operation(run.id())?;
  let evidence = contained_result(&db, &main, &checkout, &run, operation.as_ref())?;
  main.ensure(&checkout)?;
  if let (Some(evidence), Some(saved)) = (&evidence, operation.as_mut()) {
    super::direct::settle_outside(root, &mut db, saved, evidence);
  }
  Ok(evidence)
}

impl LocalIntegration {
  /// Records the merge Git proves was made without Orchester, the run's whole work in the checkout's branch, as this
  /// record's completed merge, on the person's decision (`LocalOperation::record_merged_outside`): preserved
  /// originals are then put back onto it, and a record that holds nothing else completes. Nothing is merged. Partial
  /// or equivalent evidence is refused: it stays for the person to reconcile.
  pub fn record_merged_outside(&mut self) -> Result<(), IntegrationError> {
    let actual = self.main.verified_snapshot()?;
    if actual.operation.is_some() {
      return Err(LocalError::ForeignOperation.into());
    }
    let evidence = contained_result(&self.db, &self.main, &actual, &self.run, Some(&self.operation))?
      .filter(|evidence| {
        evidence.kind == ResultEvidenceKind::CompleteAncestry && evidence.target.as_str() == actual.head
      })
      .ok_or(LocalError::Reconcile)?;
    self.operation.record_merged_outside(&evidence)?;
    self.save()
  }
}

/// Everything external evidence depends on besides Git ancestry, which never changes for the same commits: while
/// these are unchanged, an earlier answer still holds. `base` and `recorded` are the run's own, or those of a rebase
/// of its branch git proves ([`super::rebase`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceInputs {
  target: String,
  branch: String,
  marker: bool,
  base_branch: Option<String>,
  merged: bool,
  owned: Option<String>,
  recorded: Option<String>,
  branch_tip: Option<String>,
  base: String,
}

impl EvidenceInputs {
  pub(super) fn read(
    db: &ProjectDb,
    main: &LocalGit,
    checkout: &Checkout,
    run: &Run,
    operation: Option<&orchester_core::integration::local::LocalOperation>,
  ) -> Result<Self, IntegrationError> {
    let base_branch = run.base_branch().map(|branch| branch.as_str().to_owned());
    let merged = db.run_merge(run.id())?.is_some()
      || operation.is_some_and(|saved| saved.merged().is_some() || saved.merge_pending());
    let branch_tip = main.branch_tip(run.branch())?;
    // Only the target the run merges into can prove a rebase of its branch onto it.
    let (base, recorded) = if !merged && base_branch.as_deref() == Some(checkout.branch.as_str()) {
      super::rebase::effective(db, main, run, (branch_tip.as_deref(), &checkout.head))?
    } else {
      (
        run.base().as_str().to_owned(),
        run.last_checkpoint().map(|saved| saved.revision().as_str().to_owned()),
      )
    };
    Ok(Self {
      target: checkout.head.clone(),
      branch: checkout.branch.clone(),
      marker: checkout.operation.is_some(),
      base_branch,
      merged,
      owned: operation
        .and_then(|saved| saved.owned_task())
        .map(|saved| saved.head.clone()),
      recorded,
      branch_tip,
      base,
    })
  }

  /// Whether the run has a merge of Orchester's own, recorded or under way: then nothing was merged outside it.
  pub(super) const fn merged(&self) -> bool {
    self.merged
  }
}

pub(super) fn contained_result(
  db: &ProjectDb,
  main: &LocalGit,
  checkout: &Checkout,
  run: &Run,
  operation: Option<&orchester_core::integration::local::LocalOperation>,
) -> Result<Option<ContainedResult>, IntegrationError> {
  let inputs = EvidenceInputs::read(db, main, checkout, run, operation)?;
  evidence(main, &inputs)
}

/// The evidence `inputs` call for.
pub(super) fn evidence(main: &LocalGit, inputs: &EvidenceInputs) -> Result<Option<ContainedResult>, IntegrationError> {
  if inputs.merged || inputs.base_branch.as_deref() != Some(inputs.branch.as_str()) || inputs.marker {
    return Ok(None);
  }
  let inconsistent = inputs.owned.is_none()
    && inputs
      .recorded
      .as_ref()
      .zip(inputs.branch_tip.as_ref())
      .is_some_and(|(recorded, branch)| recorded != branch);
  let source = inputs
    .owned
    .clone()
    .or_else(|| inputs.recorded.clone())
    .or_else(|| inputs.branch_tip.clone());
  let unknown = source.is_none();
  let source = source.unwrap_or_else(|| inputs.base.clone());
  let kind = if unknown || inconsistent {
    ResultEvidenceKind::UnknownSource
  } else if main.is_ancestor(&source, &inputs.target)? {
    ResultEvidenceKind::CompleteAncestry
  } else if inputs.owned.is_some() {
    return Ok(None);
  } else {
    match scoped_evidence(main, &inputs.base, &source, &inputs.target)? {
      Some(kind) => kind,
      None => return Ok(None),
    }
  };
  Ok(Some(ContainedResult {
    kind,
    source: Revision::new(&source).map_err(|_| LocalError::Reconcile)?,
    target: Revision::new(&inputs.target).map_err(|_| LocalError::Reconcile)?,
  }))
}

fn scoped_evidence(
  main: &LocalGit,
  base: &str,
  source: &str,
  target: &str,
) -> Result<Option<ResultEvidenceKind>, IntegrationError> {
  if !main.is_ancestor(base, source)? {
    return Ok(Some(ResultEvidenceKind::UnknownSource));
  }
  for commit in main.commits_after(base, source)? {
    if main.is_ancestor(&commit, target)? {
      return Ok(Some(ResultEvidenceKind::PartialAncestry));
    }
  }
  if main.has_equivalent_patch(base, source, target)? {
    return Ok(Some(ResultEvidenceKind::EquivalentPatches));
  }
  let trees = main.text(&[
    "rev-parse",
    &format!("{source}^{{tree}}"),
    &format!("{target}^{{tree}}"),
  ])?;
  let mut trees = trees.lines();
  if trees.next().ok_or(LocalGitError::Data)? == trees.next().ok_or(LocalGitError::Data)? {
    return Ok(Some(ResultEvidenceKind::EquivalentTree));
  }
  Ok(None)
}
