//! A turn the provider's API refused because the CLI is too old for the step's model
//! (`orchester_core::connection::parse_cli_too_old`): the run says so instead of a bare failure,
//! and the connection records the version the model needs, so its settings and the models picked
//! for it can warn before the next run.

use orchester_core::{
  connection::{
    parse_cli_too_old,
    stream::{InvocationOutcome, RunnerProblem},
  },
  id::StepExecutionId,
  run::StepExecutor,
  store::ConnectionStore,
};

use super::{
  super::run::{ExecutionError, RunTask},
  Active,
};

impl RunTask {
  /// Reports and records a refusal in `outcome` of `active`'s invocation; nothing otherwise.
  pub(super) fn note_cli_too_old(
    &mut self,
    execution: StepExecutionId,
    active: &Active,
    outcome: &InvocationOutcome,
  ) -> Result<(), ExecutionError> {
    let InvocationOutcome::TurnFailed { message } = outcome else {
      return Ok(());
    };
    let (Some(refusal), Some(model)) = (parse_cli_too_old(message), active.invocation.model()) else {
      return Ok(());
    };
    let problem = RunnerProblem::CliTooOld {
      model: model.as_str().to_owned(),
      installed: refusal.installed,
      required: refusal.required.clone(),
    };
    self.report(&active.invocation, problem)?;
    let profile = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().step(execution.step()))
      .and_then(|step| match step.executor() {
        StepExecutor::Agent { profile, .. } => Some(*profile),
        StepExecutor::Tool { .. } => None,
      });
    let Some(profile) = profile else {
      return Ok(());
    };
    match self.profile_and_connection(&profile) {
      Ok((_, mut connection)) => {
        connection.record_model_requirement(model, &refusal.required);
        self.db.save_connection(&connection.into_connection())?;
      }
      // The run already says what the model needs; only the connection's warning is missing.
      Err(problem) => tracing::warn!(?problem, "cannot record the CLI version a model needs"),
    }
    Ok(())
  }
}
