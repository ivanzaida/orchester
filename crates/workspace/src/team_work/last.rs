//! What a task's newest team start came to, read back from the project's records. A Run that waits tells the person
//! so, and the recovery of the open starts may then settle it long after: the task's page reads the record to tell
//! what became of it, as a Run that settled at once would have.

use std::path::Path;

use orchester_core::id::TaskId;
use orchester_store::{ProjectDb, TeamStartStage};

use super::{
  StartOutcome, TeamWorkError,
  start::{KeptStart, failed, refused},
};

/// Where a task's newest start stands.
#[derive(Debug)]
pub enum LastStart {
  /// Not settled: it waits for the server or the runner, or is under way. A Run or the recovery goes on with it.
  Open,
  /// Settled: [`StartOutcome::Started`], or [`StartOutcome::Refused`] or [`StartOutcome::Failed`] for good.
  Settled(StartOutcome),
  /// The record cannot be read here, e.g. because a newer Orchester wrote it: it is never carried on by this one.
  Unreadable,
}

/// The newest start of the task `task` in the project at `root`; `None` when the task has none. Blocks on SQLite.
pub fn last_start(root: &Path, task: TaskId) -> Result<Option<LastStart>, TeamWorkError> {
  let Some(record) = ProjectDb::open(root)?.latest_team_start(task)? else {
    return Ok(None);
  };
  let read = match record.stage {
    TeamStartStage::Started => record
      .run
      .map(StartOutcome::Started)
      .ok_or(TeamWorkError::Unreadable(record.operation))
      .map(LastStart::Settled),
    TeamStartStage::Refused => refused(&record).map(LastStart::Settled),
    TeamStartStage::Failed => failed(&record).map(LastStart::Settled),
    TeamStartStage::Requested | TeamStartStage::Accepted | TeamStartStage::InputsStored => {
      serde_json::from_str::<KeptStart>(&record.request)
        .map(|_| LastStart::Open)
        .map_err(|_| TeamWorkError::Unreadable(record.operation))
    }
  };
  match read {
    Err(error) if error.is_permanent() => Ok(Some(LastStart::Unreadable)),
    read => read.map(Some),
  }
}
