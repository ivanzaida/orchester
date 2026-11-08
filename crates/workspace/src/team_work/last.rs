//! What a task's newest team start came to, read back from the project's records. A Run that waits tells the person
//! so, and the recovery of the open starts may then settle it long after: the task's page reads the record to tell
//! what became of it, as a Run that settled at once would have.

use std::path::Path;

use orchester_api::v2::EXECUTION_BUNDLE_SCHEMA;
use orchester_core::{id::TaskId, time::Timestamp};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, TeamStart, TeamStartStage};

use super::{
  StartFailure, StartOutcome, TeamWorkError,
  start::{KeptStart, failed, refused},
};

/// Where a task's newest start stands.
#[derive(Debug)]
pub enum LastStart {
  /// Not settled: it waits for the server or the runner, or is under way. A Run or the recovery goes on with it.
  Open,
  /// As [`Self::Open`], and the runner refused it the last time it was asked: the recovery asks again each round, and
  /// the page says what the runner said, as it does for a Run that met the refusal at once.
  RunnerRefused(Rejection),
  /// Settled: [`StartOutcome::Started`], or [`StartOutcome::Refused`] or [`StartOutcome::Failed`] for good, not told
  /// to the person yet.
  Settled(StartOutcome),
  /// Settled for good, and the person has seen it ([`start_seen`]), or its reason no longer holds
  /// ([`StartFailure::holds`]): the page does not tell it again in a window that asked for nothing.
  Told(StartOutcome),
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
    TeamStartStage::Refused => refused(&record).map(|outcome| settled(&record, outcome)),
    TeamStartStage::Failed => failed(&record).map(|outcome| settled(&record, outcome)),
    TeamStartStage::Requested | TeamStartStage::Accepted | TeamStartStage::InputsStored => open(&record),
  };
  match read {
    Err(error) if error.is_permanent() => Ok(Some(LastStart::Unreadable)),
    read => read.map(Some),
  }
}

/// A settled start as the page tells it: told already when the person has seen it or its reason no longer holds.
fn settled(record: &TeamStart, outcome: StartOutcome) -> LastStart {
  let lapsed = matches!(&outcome, StartOutcome::Failed(failure, _) if !failure.holds());
  if record.seen || lapsed {
    LastStart::Told(outcome)
  } else {
    LastStart::Settled(outcome)
  }
}

/// A start that is not settled: waiting, or refused by the runner the last time it was asked.
fn open(record: &TeamStart) -> Result<LastStart, TeamWorkError> {
  serde_json::from_str::<KeptStart>(&record.request).map_err(|_| TeamWorkError::Unreadable(record.operation))?;
  let Some(text) = record.runner_refusal.as_deref() else {
    return Ok(LastStart::Open);
  };
  Ok(match serde_json::from_str(text) {
    Ok(rejection) => LastStart::RunnerRefused(rejection),
    // Written by a newer Orchester: the start waits for the runner all the same, and the page says that.
    Err(error) => {
      tracing::warn!(%error, operation = %record.operation, "the runner's refusal of a team start cannot be read");
      LastStart::Open
    }
  })
}

/// Marks that the person has seen what the newest start of the task `task` came to, if it is settled for good, so the
/// task's page does not tell it again in every window ([`LastStart::Told`]). A task whose newest start is not
/// settled, or has none, is left as it is. Blocks on SQLite.
pub fn start_seen(root: &Path, task: TaskId) -> Result<(), TeamWorkError> {
  let mut db = ProjectDb::open(root)?;
  let Some(record) = db.latest_team_start(task)? else {
    return Ok(());
  };
  if matches!(record.stage, TeamStartStage::Refused | TeamStartStage::Failed) {
    db.mark_team_start_seen(record.operation, Timestamp::now())?;
  }
  Ok(())
}

impl StartFailure {
  /// Whether the reason this failure was kept for still holds on this computer: a bundle in a schema this app did not
  /// read stops being the reason once the app reads that schema, i.e. after the person updated it.
  pub fn holds(&self) -> bool {
    !matches!(self, Self::BundleSchema { built, .. } if *built == EXECUTION_BUNDLE_SCHEMA)
  }
}
