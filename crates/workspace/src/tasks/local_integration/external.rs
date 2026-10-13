use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::local::{Checkout, ContainedResult, LocalError, LocalIntegrationStore, ResultEvidenceKind},
  revision::Revision,
  run::Run,
  store::{MergeStore, RunStore},
};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;

use super::IntegrationError;

/// Positive ancestry evidence can block duplicate integration without inventing a merge event.
pub fn external_integration(root: &Path, run: RunId) -> Result<Option<ContainedResult>, IntegrationError> {
  let scratch =
    orchester_exec::local_git::prepare_scratch(root, &[".orchester", "local-integration", &run.to_string()])?;
  let _store_owner = orchester_exec::local_git::owned::StoreOwnership::acquire(root)?;
  let db = ProjectDb::open(root)?;
  let main = LocalGit::new(root, &scratch)?;
  super::ensure_project_git(&main)?;
  let checkout = main.snapshot()?;
  let run = db
    .run(run)
    .map_err(|_| IntegrationError::Run)?
    .ok_or(IntegrationError::Run)?;
  let operation = db.local_operation(run.id())?;
  let evidence = contained_result(&db, &main, &checkout, &run, operation.as_ref())?;
  main.ensure(&checkout)?;
  Ok(evidence)
}

pub(super) fn contained_result(
  db: &ProjectDb,
  main: &LocalGit,
  checkout: &Checkout,
  run: &Run,
  operation: Option<&orchester_core::integration::local::LocalOperation>,
) -> Result<Option<ContainedResult>, IntegrationError> {
  if db.run_merge(run.id())?.is_some()
    || operation.is_some_and(|saved| saved.merged().is_some() || saved.merge_pending())
    || run
      .base_branch()
      .is_none_or(|branch| branch.as_str() != checkout.branch)
    || checkout.operation.is_some()
  {
    return Ok(None);
  }
  let branch = main.branch_tip(run.branch())?;
  let owned = operation
    .and_then(|saved| saved.owned_task())
    .map(|saved| saved.head.clone());
  let recorded = run.last_checkpoint().map(|saved| saved.revision().as_str().to_owned());
  let inconsistent = owned.is_none()
    && recorded
      .as_ref()
      .zip(branch.as_ref())
      .is_some_and(|(recorded, branch)| recorded != branch);
  let source = owned.clone().or(recorded).or(branch);
  let unknown = source.is_none();
  let source = source.unwrap_or_else(|| run.base().as_str().to_owned());
  let kind = if unknown || inconsistent {
    ResultEvidenceKind::UnknownSource
  } else if main.is_ancestor(&source, &checkout.head)? {
    ResultEvidenceKind::CompleteAncestry
  } else if owned.is_some() {
    return Ok(None);
  } else {
    match scoped_evidence(main, run.base().as_str(), &source, &checkout.head)? {
      Some(kind) => kind,
      None => return Ok(None),
    }
  };
  Ok(Some(ContainedResult {
    kind,
    source: Revision::new(&source).map_err(|_| LocalError::Reconcile)?,
    target: Revision::new(&checkout.head).map_err(|_| LocalError::Reconcile)?,
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
  if main.text(&["rev-parse", &format!("{source}^{{tree}}")])?
    == main.text(&["rev-parse", &format!("{target}^{{tree}}")])?
  {
    return Ok(Some(ResultEvidenceKind::EquivalentTree));
  }
  Ok(None)
}
