//! Legacy data-scope merging keeps safe metadata and outcomes without manufacturing a local decision.
use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  id::RunId,
  integration::{
    RunMerge,
    local::{ContainedResult, LocalIntegrationStore, LocalOperation},
  },
  store::RunStore,
};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;

use super::{IntegrationError, view::Redaction};
use crate::{messages::Message, secrets::Vault};

#[derive(Clone)]
pub struct LegacyPolicy {
  root: PathBuf,
  run: RunId,
  vault: Vault,
  main: LocalGit,
  _owner: Arc<orchester_exec::local_git::owned::StoreOwnership>,
}
impl LegacyPolicy {
  pub fn new(root: &Path, run: RunId, vault: Vault) -> Result<Self, IntegrationError> {
    let owner = orchester_exec::local_git::owned::StoreOwnership::acquire(root)?;
    let scratch =
      orchester_exec::local_git::prepare_scratch(root, &[".orchester", "local-integration", &run.to_string()])?;
    Ok(Self {
      root: root.to_owned(),
      run,
      vault,
      main: LocalGit::new(root, &scratch)?,
      _owner: owner,
    })
  }
  pub fn preflight(&self) -> Result<Option<ContainedResult>, IntegrationError> {
    let db = ProjectDb::open(&self.root)?;
    if managed_checkout(&db, self.run)? {
      return Err(super::LocalError::ApprovalRequired.into());
    }
    let run = db
      .run(self.run)
      .map_err(|_| IntegrationError::Run)?
      .ok_or(IntegrationError::Run)?;
    let checkout = self.main.snapshot()?;
    let policy = Redaction::load(&db, self.vault.get())?;
    policy.ensure_metadata(&checkout)?;
    policy.ensure_text_metadata(db.project().name().as_str())?;
    policy.ensure_text_metadata(run.branch().as_str())?;
    if let Some(branch) = run.base_branch() {
      policy.ensure_text_metadata(branch.as_str())?;
    }
    if let Some(source) = self.main.branch_tip(run.branch())? {
      policy.ensure_tree_metadata(&self.main, &source)?;
    }
    let evidence = super::external::contained_result(&db, &self.main, &checkout, &run, None)?;
    self.main.ensure(&checkout)?;
    Ok(evidence)
  }
  pub fn safe_message(&self, message: Message) -> Message {
    match super::metadata_allowed(
      &self.root,
      &message.args.iter().map(|(_, value)| value.clone()).collect::<Vec<_>>(),
      self.vault.get(),
    ) {
      Ok(true) => message,
      _ => Message::new("localIntegration.sensitiveOutcome"),
    }
  }
  pub fn safe_paths(&self, paths: Vec<String>) -> Vec<String> {
    match super::metadata_allowed(&self.root, &paths, self.vault.get()) {
      Ok(true) => paths,
      _ => Vec::new(),
    }
  }
}
/// Whether a local integration holds the shared checkout: the run's own or another run's that started work it must
/// finish or put back. A review that only read the checkout, also one paused since, holds nothing and does not stand
/// in the way of the run's direct merge; historical finished records do not reserve the checkout forever.
pub(crate) fn managed_checkout(db: &ProjectDb, run: RunId) -> Result<bool, orchester_store::StoreError> {
  Ok(checkout_holder(db, run)?.is_some())
}

/// Which local integration holds the shared checkout, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckoutHolder {
  /// The run's own.
  ThisRun,
  /// Another run's.
  OtherRun,
}

pub(crate) fn checkout_holder(
  db: &ProjectDb,
  run: RunId,
) -> Result<Option<CheckoutHolder>, orchester_store::StoreError> {
  let holders: Vec<RunId> = db
    .local_operations()?
    .iter()
    .filter(|operation| operation.reserves_checkout())
    .map(LocalOperation::run)
    .collect();
  Ok(if holders.contains(&run) {
    Some(CheckoutHolder::ThisRun)
  } else if holders.is_empty() {
    None
  } else {
    Some(CheckoutHolder::OtherRun)
  })
}

/// Marks the run's own local integration complete once the direct merge recorded `merge`, where that integration
/// only reviewed the checkout: its tab then shows the merge instead of a review paused before it. Opening the tab
/// reconciles the same way, so a review that cannot be settled now (the store refused) costs nothing but a log line;
/// the merge stands either way.
pub(crate) fn settle_review(db: &mut ProjectDb, merge: &RunMerge) {
  let settled = (|| -> Result<(), IntegrationError> {
    let Some(mut operation) = db.local_operation(merge.run)? else {
      return Ok(());
    };
    if operation.reserves_checkout() || operation.merged().is_some() {
      return Ok(());
    }
    let version = operation.version();
    operation.reconcile_recorded_merge(merge)?;
    operation.advance_version();
    if !db.save_local_operation(&operation, Some(version))? {
      return Err(super::LocalError::Stale.into());
    }
    Ok(())
  })();
  if let Err(error) = settled {
    tracing::warn!(%error, "the run was merged, but its local review could not be marked complete");
  }
}
