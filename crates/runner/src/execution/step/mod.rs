//! One agent step: the invocation record, the Codex CLI in the run's worktree (a new session, or
//! the step's earlier session continued), its output parsed into the transcript as it arrives,
//! its usage checked against the run's limits, and a checkpoint commit of what it changed.

mod command;
mod stream;

use std::time::Duration;

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, RunnerProblem},
  id::StepExecutionId,
  resource::ModelId,
  revision::Revision,
  run::{Invocation, RunLimit, StepExecutor},
  store::ActivityStore,
  time::Timestamp,
};

use self::stream::Stop;
use super::{
  checkpoint,
  run::{ExecutionError, RunTask},
};
use crate::supervisor::Supervised;

/// How a step's work ended.
pub(super) enum StepEnd {
  /// The agent finished; `checkpoint` is the commit of its changes, if it changed anything.
  Passed {
    checkpoint: Option<Revision>,
  },
  Failed,
  /// A cancel or the runner stopping ended the agent; the request is still to be carried out.
  Stopped,
  /// The run reached this limit while the agent worked; the agent was ended mid-step.
  Limit(RunLimit),
}

impl RunTask {
  /// Runs `execution`'s agent: continuing session `resume` when given, for at most `time_left`
  /// of active time.
  pub(super) async fn execute(
    &mut self,
    execution: StepExecutionId,
    resume: Option<String>,
    time_left: Duration,
  ) -> Result<StepEnd, ExecutionError> {
    let model = self.model(execution)?;
    let mut invocation = self
      .db
      .start_invocation(self.run.id(), execution, &model, Timestamp::now())?;
    if resume.is_some() {
      invocation.session.clone_from(&resume);
      self.db.save_invocation(&invocation)?;
    }
    let command = match self.command(execution, resume.as_deref()) {
      Ok(command) => command,
      Err(problem) => return self.not_started(&mut invocation, problem),
    };
    let process = match Supervised::start(&command) {
      Ok(process) => process,
      Err(error) => {
        let problem = RunnerProblem::CouldNotStart {
          detail: error.to_string(),
        };
        return self.not_started(&mut invocation, problem);
      }
    };
    let (outcome, stop) = self.stream(&mut invocation, process, time_left).await?;
    let succeeded = outcome.is_success();
    self.finish(&mut invocation, outcome)?;
    match stop {
      Some(Stop::Requested) => return Ok(StepEnd::Stopped),
      Some(Stop::Limit(limit)) => return Ok(StepEnd::Limit(limit)),
      None if !succeeded => return Ok(StepEnd::Failed),
      None => {}
    }
    match checkpoint::commit(&self.worktree, &self.commit_message(execution)) {
      Ok(checkpoint) => Ok(StepEnd::Passed { checkpoint }),
      Err(error) => {
        let problem = RunnerProblem::CheckpointFailed {
          detail: error.to_string(),
        };
        self.record_activity(
          &invocation,
          &[(Timestamp::now(), AgentEvent::RunnerProblem { problem })],
        )?;
        Ok(StepEnd::Failed)
      }
    }
  }

  fn model(&self, execution: StepExecutionId) -> Result<ModelId, ExecutionError> {
    let step = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().step(execution.step()));
    match step.map(|step| step.executor()) {
      Some(StepExecutor::Agent { model, .. }) => Ok(model.clone()),
      _ => Err(ExecutionError::NotAnAgentStep(execution)),
    }
  }

  fn not_started(&mut self, invocation: &mut Invocation, problem: RunnerProblem) -> Result<StepEnd, ExecutionError> {
    tracing::warn!(run = %self.run.id(), ?problem, "the agent did not start");
    self.record_activity(invocation, &[(Timestamp::now(), AgentEvent::RunnerProblem { problem })])?;
    self.finish(invocation, InvocationOutcome::NotStarted)?;
    Ok(StepEnd::Failed)
  }

  fn finish(&mut self, invocation: &mut Invocation, outcome: InvocationOutcome) -> Result<(), ExecutionError> {
    invocation.ended_at = Some(Timestamp::now());
    invocation.outcome = Some(outcome);
    Ok(self.db.save_invocation(invocation)?)
  }

  /// "ORC-129: Implement (run 128)".
  fn commit_message(&self, execution: StepExecutionId) -> String {
    let step = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().body().step(execution.step()))
      .map_or_else(String::new, |step| step.name.as_str().to_owned());
    format!("{}: {step} (run {})", self.task.key(), self.run.number())
  }
}
