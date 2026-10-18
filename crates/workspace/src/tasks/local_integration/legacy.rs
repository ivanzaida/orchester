//! Legacy data-scope merging keeps safe metadata and outcomes without manufacturing a local decision.
use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{id::RunId, integration::local::ContainedResult, store::RunStore};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;

use super::{IntegrationError, direct::managed_checkout, view::Redaction};
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
