//! A run whose task failed on the runner's own side: the project database failed or refused a write, a
//! transition was rejected, or no step could run. Its agents have ended by then. The run must not stay
//! running with nothing behind it, so it is interrupted ([`InterruptCause::RunnerFailed`]) from what was
//! last stored, and the error goes into its activity: a person continues it from its last checkpoint,
//! where retrying is safe, or cancels it.

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  run::{InterruptCause, RunStatus},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;

use super::{
  record::record,
  run::{ExecutionError, RunTask},
};

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
    if let Some(stored) = self.db.run(run).map_err(super::record::RecordError::from)? {
      self.run = stored;
    }
    if self.run.status() != &RunStatus::Running {
      return Ok(());
    }
    let problem = RunnerProblem::RunnerFailed {
      detail: error.to_string(),
    };
    if let Some(invocation) = self.db.invocations(run)?.last() {
      let events = [(Timestamp::now(), AgentEvent::RunnerProblem { problem })];
      let sequence = self.db.append_activity(invocation, &events)?;
      self.notifier.send(Event::AgentActivity { run, sequence });
    }
    record(&mut self.db, &self.notifier, &mut self.run, |run, now| {
      run.interrupt(InterruptCause::RunnerFailed, now)
    })?;
    Ok(())
  }
}
