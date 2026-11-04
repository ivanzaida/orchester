//! Recording a run's transitions: the events are sent to the coordinator's sink with the new state,
//! then the desktop is told that the run changed.

use orchester_core::{
  run::{Run, RunError, RunEvent, RunStatus},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{RunLogRecord, RunStoreError};
use thiserror::Error;

use super::{Notifier, sink::Sink};

#[derive(Debug, Error)]
pub(super) enum RecordError {
  #[error(transparent)]
  Run(#[from] RunError),
  #[error(transparent)]
  Store(#[from] RunStoreError),
}

/// Tells the desktop that `run` is ready for review, if it succeeded.
pub(super) fn announce_ready(notifier: &Notifier, run: &Run) {
  if run.status() == &RunStatus::Succeeded {
    notifier.send(Event::RunReady {
      run: run.id(),
      task: run.task(),
      number: run.number(),
    });
  }
}

/// Applies `transition` to `run`, sends its events with the new state to `sink` and announces the
/// change. A transition that changes nothing records nothing.
pub(super) fn record(
  sink: &mut dyn Sink,
  notifier: &Notifier,
  run: &mut Run,
  transition: impl FnOnce(&mut Run, Timestamp) -> Result<Vec<RunEvent>, RunError>,
) -> Result<(), RecordError> {
  let events = transition(run, Timestamp::now())?;
  if events.is_empty() {
    return Ok(());
  }
  sink.run_log(RunLogRecord::events(run, &events)?)?;
  notifier.send(Event::RunChanged {
    run: run.id(),
    task: run.task(),
  });
  Ok(())
}
