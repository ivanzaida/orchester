//! The failure a retry attempt's prompt reports ([`orchester_core::run::Run::retry_report`] decides
//! which executions get one), read from what the failed execution left: an agent's structured result
//! or final message, or how a tool step's command ended and what it printed.

use std::num::NonZeroU32;

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem, ToolCommandResult},
  id::StepExecutionId,
  resource::StepKey,
  run::{FailureReport, RetryReport, ReviewNote, StepExecutor, StepOutput, StepResult},
  store::ActivityStore,
};

use super::{super::super::run::RunTask, problem};

/// What the failed execution left, read for the prompt.
pub(super) struct LoadedReport {
  failed_step: StepKey,
  attempt: NonZeroU32,
  note: Option<ReviewNote>,
  failure: LoadedFailure,
}

enum LoadedFailure {
  Result(StepResult),
  Message(String),
  Command {
    command: String,
    result: ToolCommandResult,
    exit_code: Option<i64>,
    output: String,
    omitted_bytes: u64,
  },
}

impl LoadedReport {
  /// The report as the prompt takes it.
  pub(super) fn report<'a>(&'a self, task: &'a RunTask) -> RetryReport<'a> {
    let failure = match &self.failure {
      LoadedFailure::Result(result) => FailureReport::Result(result),
      LoadedFailure::Message(message) => FailureReport::Message(message),
      LoadedFailure::Command {
        command,
        result,
        exit_code,
        output,
        omitted_bytes,
      } => FailureReport::Command {
        command,
        result: *result,
        exit_code: *exit_code,
        output,
        omitted_bytes: *omitted_bytes,
      },
    };
    RetryReport {
      failed_step: task
        .run
        .snapshot()
        .body()
        .step(&self.failed_step)
        .expect("the failed execution names a step of the run's snapshot"),
      attempt: self.attempt,
      failure,
      note: self.note.as_ref(),
    }
  }
}

impl RunTask {
  /// The failure `execution`'s prompt reports, if its attempt is a retry that reports one to it.
  pub(super) fn retry_report(
    &self,
    execution: StepExecutionId,
    outputs: &[StepOutput],
  ) -> Result<Option<LoadedReport>, RunnerProblem> {
    let Some(source) = self.run.retry_report(execution) else {
      return Ok(None);
    };
    let failed = source.failed.id();
    let tool = matches!(
      self
        .run
        .snapshot()
        .step(source.failed.step())
        .map(|step| step.executor()),
      Some(StepExecutor::Tool { .. })
    );
    let failure = if tool {
      self.command_failure(failed, outputs)?
    } else {
      self.agent_failure(failed, outputs)?
    };
    Ok(Some(LoadedReport {
      failed_step: source.failed.step().clone(),
      attempt: source.attempt,
      note: source.note.cloned(),
      failure,
    }))
  }

  /// An agent's structured result, else its final message, else what it left for the steps after it.
  fn agent_failure(&self, failed: StepExecutionId, outputs: &[StepOutput]) -> Result<LoadedFailure, RunnerProblem> {
    let message = self
      .db
      .last_agent_message(self.run.id(), failed)
      .map_err(|error| unreadable(&error))?
      .map(|(_, message)| message);
    Ok(match message {
      Some(message) => match StepResult::parse(&message) {
        Some(result) => LoadedFailure::Result(result),
        None => LoadedFailure::Message(message),
      },
      None => LoadedFailure::Message(output_text(failed, outputs)),
    })
  }

  /// How the command ended and what it printed, else what the step left for the steps after it. Its
  /// output reaches another agent's prompt unredacted, which is safe while tools get no secrets
  /// (see `orchester_core::run::FailureReport`).
  fn command_failure(&self, failed: StepExecutionId, outputs: &[StepOutput]) -> Result<LoadedFailure, RunnerProblem> {
    let finished = self
      .db
      .last_command_finished(self.run.id(), failed)
      .map_err(|error| unreadable(&error))?;
    Ok(match finished {
      Some(AgentEvent::ToolCommandFinished {
        command,
        result,
        exit_code,
        output,
        omitted_bytes,
        ..
      }) => LoadedFailure::Command {
        command,
        result,
        exit_code,
        output,
        omitted_bytes,
      },
      _ => LoadedFailure::Message(output_text(failed, outputs)),
    })
  }
}

fn output_text(execution: StepExecutionId, outputs: &[StepOutput]) -> String {
  outputs
    .iter()
    .find(|output| output.execution == execution)
    .map(|output| output.text.clone())
    .unwrap_or_default()
}

fn unreadable(error: &dyn std::error::Error) -> RunnerProblem {
  problem(format!("cannot read the failed step's report: {error}"))
}
