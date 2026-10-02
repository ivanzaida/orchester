//! What happens when a command ends: as for an agent (see `step::end`), its work is committed and a
//! step that passed joins the run's branch; its summary is the step's output, and an exit code the
//! tool does not expect or a timeout fails the step for its failure route to handle.

use orchester_core::{id::StepExecutionId, store::ActivityStore};

use super::CommandFinished;
use crate::execution::{
  run::{ExecutionError, RunTask},
  step::worker::StopCause,
};

impl RunTask {
  /// `execution`'s command ended as `finished` says.
  pub(in super::super) fn on_command_ended(
    &mut self,
    execution: StepExecutionId,
    finished: &CommandFinished,
  ) -> Result<(), ExecutionError> {
    let Some(active) = self.agents.remove(&execution) else {
      return Ok(());
    };
    self.board.release(execution);
    if active.abandoned {
      return Ok(());
    }
    match finished.stopped {
      // The execution stays open: its command runs again once the run continues.
      Some(StopCause::Limit(_)) => self.save(execution, &active, "work in progress at a limit"),
      Some(StopCause::Paused) => self.save(execution, &active, "work in progress at a pause"),
      // Carried out once every agent and command has ended.
      Some(StopCause::Requested | StopCause::Abandoned | StopCause::Permissions) => Ok(()),
      None => {
        if let Some(output) = &finished.output {
          self.db.save_step_output(output)?;
        }
        match finished.failure {
          Some(failure) => {
            self.save(execution, &active, "step failed")?;
            self.fail(execution, failure)
          }
          None => {
            let output = finished.output.as_ref().map(|output| output.text.clone());
            self.pass(execution, &active, output.unwrap_or_default())
          }
        }
      }
    }
  }
}
