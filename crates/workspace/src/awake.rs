//! When the computer is kept awake (`docs/plan/orchestrator-autonomy/OA-08-…`): while work is under way in a
//! project, which is while an agent of one of its runs works or a turn of its Orchestrator chat thinks. A run that
//! waits for a person, is paused, stopped at a limit, interrupted or ended is not at work: nothing would be lost by
//! the computer sleeping then. The rule is this crate's; the app holds the system awake by it and lets go at once
//! (`orchester_exec::awake`), only while the person's setting is on.

use std::path::Path;

use orchester_core::{run::at_work, store::RunStore};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::detail::run_store_error;

/// Whether work is under way in the project at `root` now. Blocks on SQLite.
pub fn work_under_way(root: &Path) -> Result<bool, StoreError> {
  let db = ProjectDb::open_read_only(root)?;
  if db.any_chat_turn_thinks()? {
    return Ok(true);
  }
  for run in db.active_task_runs()?.into_values() {
    // It runs, and no step of it waits for a person.
    if db.run(run).map_err(run_store_error)?.as_ref().is_some_and(at_work) {
      return Ok(true);
    }
  }
  Ok(false)
}
