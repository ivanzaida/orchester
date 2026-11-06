//! Runner notes a step's tools write into the run's transcript while the agent works, off the run's own task: a QA
//! start that waits for a slot, a service that crashed. They go through the coordinator's sink, and the desktop is
//! told the activity changed.

use std::path::Path;

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::{InvocationId, RunId},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{ActivityRecord, ProjectDb, StoreError};

use crate::execution::{Notifier, executor::sink::Sink};

/// Notes `problem` in the transcript of `invocation` of `run`, in the project at `root`, through `sink`. A note that
/// cannot be written is logged: what it tells goes on all the same.
pub(super) fn note(
  (root, sink): (&Path, &mut dyn Sink),
  (run, invocation): (RunId, InvocationId),
  problem: RunnerProblem,
  notifier: &Notifier,
) {
  let noted = || -> Result<Option<u64>, StoreError> {
    let db = ProjectDb::open(root)?;
    let Some(invocation) = db.invocations(run)?.into_iter().find(|found| found.id == invocation) else {
      return Ok(None);
    };
    let note = AgentEvent::RunnerProblem { problem };
    sink
      .activity(ActivityRecord::new(&invocation, vec![(Timestamp::now(), note)]))
      .map(Some)
  };
  match noted() {
    Ok(Some(sequence)) => notifier.send(Event::AgentActivity { run, sequence }),
    Ok(None) => tracing::warn!(%run, %invocation, "the invocation of a runner note is not recorded"),
    Err(error) => tracing::warn!(%run, %error, "a runner note was not written in the transcript"),
  }
}
