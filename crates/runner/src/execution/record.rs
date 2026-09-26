//! Recording a run's transitions: the events are appended with the new state, then the desktop
//! is told that the run changed.

use orchester_core::{
  run::{Run, RunError, RunEvent},
  store::RunStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{ProjectDb, RunStoreError};
use thiserror::Error;

use super::Notifier;

#[derive(Debug, Error)]
pub(super) enum RecordError {
  #[error(transparent)]
  Run(#[from] RunError),
  #[error(transparent)]
  Store(#[from] RunStoreError),
}

/// Applies `transition` to `run`, stores its events with the new state and announces the change.
/// A transition that changes nothing records nothing.
pub(super) fn record(
  db: &mut ProjectDb,
  notifier: &Notifier,
  run: &mut Run,
  transition: impl FnOnce(&mut Run, Timestamp) -> Result<Vec<RunEvent>, RunError>,
) -> Result<(), RecordError> {
  let events = transition(run, Timestamp::now())?;
  if events.is_empty() {
    return Ok(());
  }
  db.append_events(run, &events)?;
  notifier.send(Event::RunChanged {
    run: run.id(),
    task: run.task(),
  });
  Ok(())
}
