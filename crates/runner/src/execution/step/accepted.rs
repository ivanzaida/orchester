//! A step a person accepted as it is (`StuckDecision::Accept`): the attempt the acceptance started has a fresh
//! execution of it, which runs no agent. It passes as a step its agent finished passes ([`RunTask::pass`]), with the
//! work and the result of the execution accepted (`Run::accepted_execution`): that one's own worktree, where its work
//! was committed when it ended, is merged into the run's branch; a gated step proposes the accepted result as its plan
//! and waits for the decision; and the result becomes the new execution's output for the steps after it. Problems are
//! reported around the accepted execution's last invocation, whose work it is.

use orchester_core::{
  connection::stream::RunnerProblem,
  id::StepExecutionId,
  memory::StepFate,
  run::{StepFailure, StepOutput},
  store::ActivityStore,
  time::Timestamp,
};

use super::super::{
  run::{ExecutionError, RunTask},
  workspace::Workspace,
};

impl RunTask {
  /// Passes `execution` when it is the execution of an accepted step that the acceptance started; `false` for any
  /// other execution, whose agent starts.
  pub(super) fn passes_accepted(&mut self, execution: StepExecutionId) -> Result<bool, ExecutionError> {
    let Some(accepted) = self.run.accepted_execution(execution) else {
      return Ok(false);
    };
    self.pass_accepted(execution, accepted)?;
    Ok(true)
  }

  /// Passes `execution` with the work and result of `accepted`.
  fn pass_accepted(&mut self, execution: StepExecutionId, accepted: StepExecutionId) -> Result<(), ExecutionError> {
    let invocation = self
      .last_invocation(accepted)?
      .ok_or(ExecutionError::NotAnAgentStep(accepted))?;
    self.transition(|run, now| run.start_step(execution, now))?;
    let judged = self
      .run
      .execution(accepted)
      .cloned()
      .ok_or(ExecutionError::NotAnAgentStep(accepted))?;
    // Checked when the person accepted; checked again, as the work may have changed since.
    let workspace = match Workspace::accepted_work(&self.root, (&self.run, &self.worktree), &judged) {
      Ok(workspace) => workspace,
      Err(detail) => {
        let problem = RunnerProblem::CheckpointFailed { detail };
        self.report(&invocation, problem)?;
        self.keep_handoff(execution, StepFate::Failed);
        return self.fail(execution, StepFailure::CheckpointFailed);
      }
    };
    let result = self
      .db
      .step_outputs(self.run.id())?
      .into_iter()
      .find(|output| output.execution == accepted);
    let text = result.as_ref().map(|result| result.text.clone()).unwrap_or_default();
    if let Some(result) = result {
      self.db.save_step_output(&StepOutput {
        execution,
        created_at: Timestamp::now(),
        ..result
      })?;
    }
    self.pass(execution, (&workspace, &invocation), text)
  }
}
