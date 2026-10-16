//! When a run's states began, read from its event log: when it paused or stopped at a limit (and whether its work
//! was saved right before), when each attempt started, and when it ended.

use orchester_core::{
  run::{AttemptStart, Run, RunEvent, RunEventKind, RunStatus, attempt_starts},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::detail::run_store_error;

/// When a paused run paused or stopped at its limit, and whether its work was committed as a
/// checkpoint right before.
pub(super) fn paused_at(db: &ProjectDb, run: &Run) -> Result<(Option<Timestamp>, bool), StoreError> {
  if !matches!(run.status(), RunStatus::Paused(_)) {
    return Ok((None, false));
  }
  let events = db.events(run.id()).map_err(run_store_error)?;
  let paused = events
    .iter()
    .rposition(|event| matches!(event.kind(), RunEventKind::Paused { .. }));
  let saved = paused
    .is_some_and(|index| index > 0 && matches!(events[index - 1].kind(), RunEventKind::CheckpointRecorded { .. }));
  Ok((paused.map(|index| events[index].at()), saved))
}

/// When each attempt of `run` started and who started it; nothing to read for a run of one attempt.
pub(super) fn starts_of(db: &ProjectDb, run: &Run) -> Result<Vec<AttemptStart>, StoreError> {
  if run.attempts().len() < 2 {
    return Ok(Vec::new());
  }
  Ok(attempt_starts(&db.events(run.id()).map_err(run_store_error)?))
}

/// When `run` ended, if it did: succeeded, failed or was cancelled.
pub(crate) fn finished_at(db: &ProjectDb, run: &Run) -> Result<Option<Timestamp>, StoreError> {
  if !run.status().is_terminal() {
    return Ok(None);
  }
  let events = db.events(run.id()).map_err(run_store_error)?;
  Ok(
    events
      .iter()
      .rev()
      .find(|event| {
        matches!(
          event.kind(),
          RunEventKind::Succeeded | RunEventKind::Failed { .. } | RunEventKind::Cancelled
        )
      })
      .map(RunEvent::at),
  )
}
