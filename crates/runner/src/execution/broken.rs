//! A run whose task failed on the runner's own side: the project database failed or refused a write, a
//! transition was rejected, or no step could run. Its agents have ended by then. The run must not stay
//! running with nothing behind it, so it is interrupted ([`InterruptCause::RunnerFailed`]) from what was
//! last stored, with why ([`RunnerFailure`], ORC-75), and the error goes into its activity: a person continues
//! it from its last checkpoint, where retrying is safe, or cancels it; the Orchestrator may continue it too when
//! the database only stayed busy.

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, RunnerProblem},
  id::RunId,
  run::{InterruptCause, RunStatus, RunnerFailure},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{ProjectDb, StoreError};

use super::{
  record::RecordError,
  run::{ExecutionError, RunTask},
};

impl ExecutionError {
  /// What kind of failure stopped the run, as the store reads its storage errors.
  fn runner_failure(&self) -> RunnerFailure {
    match self {
      Self::Store(error) => error.runner_failure(),
      Self::Record(RecordError::Store(error)) => error.runner_failure(),
      Self::Record(RecordError::Run(_)) | Self::NotAnAgentStep(_) | Self::Stalled | Self::Conversation(_) => {
        RunnerFailure::Other
      }
    }
  }
}

impl RunTask {
  /// Interrupts the run after `error` ended its task. What the task held in memory may be ahead of
  /// what was stored, so the run is read again, through a fresh connection. A run that settled
  /// meanwhile stays as it is; one that cannot be recorded either is left to the runner's log.
  pub(super) fn interrupt_broken(&mut self, error: &ExecutionError) {
    if let Err(second) = self.try_interrupt(error) {
      tracing::error!(run = %self.run.id(), %second, "the run could not be interrupted after its failure");
    }
  }

  fn try_interrupt(&mut self, error: &ExecutionError) -> Result<(), ExecutionError> {
    let run = self.run.id();
    self.db = ProjectDb::open(&self.root)?;
    if let Some(stored) = self.db.run(run).map_err(RecordError::from)? {
      self.run = stored;
    }
    if self.run.status() != &RunStatus::Running {
      return Ok(());
    }
    // Recording an invocation or activity may still fail. Persist the interruption first, with why, so
    // a writable run log never claims that the workers we already ended are still running.
    let ended_at = Timestamp::now();
    let events = self
      .run
      .interrupt(InterruptCause::RunnerFailed, ended_at)
      .map_err(RecordError::from)?;
    self
      .db
      .append_runner_failure(&self.run, &events, error.runner_failure())
      .map_err(RecordError::from)?;
    self.notifier.send(Event::RunChanged {
      run,
      task: self.run.task(),
    });
    if let Err(error) = close_invocations(&mut self.db, run, ended_at) {
      tracing::error!(%run, %error, "the ended invocations could not be recorded after runner failure");
    }
    let problem = RunnerProblem::RunnerFailed {
      detail: error.to_string(),
    };
    if let Some(invocation) = self.db.invocations(run)?.last() {
      let events = [(Timestamp::now(), AgentEvent::RunnerProblem { problem })];
      let sequence = self.db.append_activity(invocation, &events)?;
      self.notifier.send(Event::AgentActivity { run, sequence });
    }
    Ok(())
  }
}

/// All workers had ended by the recorded interruption. Close missing end records then, even
/// if storage only becomes writable on an explicit continuation, preserving usage and sessions.
pub(super) fn close_invocations(db: &mut ProjectDb, run: RunId, ended_at: Timestamp) -> Result<(), StoreError> {
  for mut invocation in db
    .invocations(run)?
    .into_iter()
    .filter(|invocation| invocation.ended_at.is_none())
  {
    invocation.ended_at = Some(ended_at);
    invocation.outcome = Some(InvocationOutcome::Unrecorded);
    invocation.timing.resume(ended_at);
    db.save_invocation(&invocation)?;
  }
  Ok(())
}
