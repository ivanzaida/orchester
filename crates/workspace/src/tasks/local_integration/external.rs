use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::local::{Checkout, ContainedResult, LocalError, LocalIntegrationStore},
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
  let Some(source) = main.branch_tip(run.branch())? else {
    return Ok(None);
  };
  if !main.is_ancestor(&source, &checkout.head)? {
    return Ok(None);
  }
  Ok(Some(ContainedResult {
    source: Revision::new(&source).map_err(|_| LocalError::Reconcile)?,
    target: Revision::new(&checkout.head).map_err(|_| LocalError::Reconcile)?,
  }))
}
