use std::path::Path;

use orchester_core::{id::TaskId, integration::local::LocalIntegrationStore, store::RunStore};
use orchester_store::ProjectDb;

use super::IntegrationError;
use crate::tasks::detail::{RunView, run_view};

/// Keep a task's reserved operation reachable after a newer run supersedes it.
pub fn recovery_run(root: &Path, task: TaskId) -> Result<Option<RunView>, IntegrationError> {
  let db = ProjectDb::open(root)?;
  for operation in db.local_operations()? {
    if !operation.reserves_checkout() {
      continue;
    }
    let run = db
      .run(operation.run())
      .map_err(|_| IntegrationError::Run)?
      .ok_or(IntegrationError::Run)?;
    if run.task() == task {
      return Ok(Some(run_view(&db, run)?));
    }
  }
  Ok(None)
}
