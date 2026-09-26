//! One agent step: the invocation record, the prompt, the Codex CLI in the run's worktree, its
//! output parsed into the transcript as it arrives, and a checkpoint commit of what it changed.

use std::{path::Path, time::Duration};

use orchester_core::{
  command::Command,
  connection::{
    REASONING_EFFORT,
    invocation::{CodexInvocation, Platform},
    stream::{AgentEvent, InvocationOutcome, InvocationTracker, RunnerProblem, parse_codex_line},
  },
  id::StepExecutionId,
  project::Project,
  resource::{AgentProfile, ModelId, StepKind},
  revision::Revision,
  run::{InterruptCause, Invocation, PipelineInfo, PromptAssemblyError, PromptInput, Run, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore},
  task::Task,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;
use tokio::sync::watch;

use super::Notifier;
use crate::{
  git::Git,
  supervisor::{ProcessEvent, ProcessExit, Supervised},
  worktree::WorktreeError,
};

/// Transcript entries are written at least this often while the CLI prints…
const FLUSH_INTERVAL: Duration = Duration::from_millis(250);
/// …or as soon as this many are waiting.
const FLUSH_BATCH: usize = 64;

#[derive(Debug, Error)]
pub(super) enum StepError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("step execution {0} is not an agent step of the run")]
  NotAnAgentStep(StepExecutionId),
}

/// How a step's work ended.
pub(super) enum StepEnd {
  /// The agent finished; `checkpoint` is the commit of its changes, if it changed anything.
  Passed {
    checkpoint: Option<Revision>,
  },
  Failed,
  /// The runner was asked to stop; the agent was ended.
  Stopped(InterruptCause),
}

pub(super) struct StepRun<'a> {
  pub(super) db: &'a mut ProjectDb,
  pub(super) run: &'a Run,
  pub(super) task: &'a Task,
  pub(super) project: &'a Project,
  pub(super) worktree: &'a Path,
  pub(super) notifier: &'a Notifier,
  pub(super) stop: &'a mut watch::Receiver<Option<InterruptCause>>,
}

impl StepRun<'_> {
  pub(super) async fn execute(mut self, execution: StepExecutionId) -> Result<StepEnd, StepError> {
    let model = self.model(execution)?;
    let mut invocation = self
      .db
      .start_invocation(self.run.id(), execution, &model, Timestamp::now())?;
    let command = match self.command(execution) {
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
    let (outcome, stopped) = self.stream(&mut invocation, process).await?;
    let succeeded = outcome.is_success();
    self.finish(&mut invocation, outcome)?;
    if let Some(cause) = stopped {
      return Ok(StepEnd::Stopped(cause));
    }
    if !succeeded {
      return Ok(StepEnd::Failed);
    }
    match self.commit(execution) {
      Ok(checkpoint) => Ok(StepEnd::Passed { checkpoint }),
      Err(error) => {
        let problem = RunnerProblem::CheckpointFailed {
          detail: error.to_string(),
        };
        self.record(
          &invocation,
          &[(Timestamp::now(), AgentEvent::RunnerProblem { problem })],
        )?;
        Ok(StepEnd::Failed)
      }
    }
  }

  fn model(&self, execution: StepExecutionId) -> Result<ModelId, StepError> {
    let step = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().step(execution.step()));
    match step.map(|step| step.executor()) {
      Some(StepExecutor::Agent { model, .. }) => Ok(model.clone()),
      _ => Err(StepError::NotAnAgentStep(execution)),
    }
  }

  /// The Codex command of the step, or why it cannot start.
  fn command(&self, execution: StepExecutionId) -> Result<Command, RunnerProblem> {
    let problem = |detail: String| RunnerProblem::CouldNotStart { detail };
    let key = self
      .run
      .execution(execution)
      .expect("the execution's model was found")
      .step();
    let step = self
      .run
      .snapshot()
      .body()
      .step(key)
      .expect("executions name snapshot steps");
    let (StepKind::Agent { repository, .. }, Some(StepExecutor::Agent { profile, model, .. })) =
      (&step.kind, self.run.snapshot().step(key).map(|step| step.executor()))
    else {
      return Err(problem(format!("step `{key}` is not an agent step")));
    };
    let body: AgentProfile = self
      .db
      .version::<AgentProfile>(profile.resource, profile.number)
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem(format!("agent profile version {} is missing", profile.number)))?
      .body()
      .clone();
    let connection = self
      .db
      .connection(body.connection())
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem("the agent profile's connection no longer exists".to_owned()))?;
    let pipeline = self.run.snapshot().pipeline();
    let pipeline_name = self
      .db
      .resource(pipeline.resource)
      .map_err(|error| problem(error.to_string()))?
      .map(|resource| resource.name().as_str().to_owned())
      .ok_or_else(|| problem("the run's pipeline no longer exists".to_owned()))?;
    let prompt = PromptInput {
      project: self.project,
      task: self.task,
      run: self.run,
      execution,
      pipeline: PipelineInfo {
        id: pipeline.resource,
        name: &pipeline_name,
        version: pipeline.number,
      },
      step,
      profile_id: profile.resource,
      profile: &body,
    }
    .assemble()
    .map_err(
      |PromptAssemblyError::UnavailableVariable { variable, .. }| RunnerProblem::UnavailableVariable {
        variable: variable.name().to_owned(),
      },
    )?;
    let effort = body
      .settings()
      .iter()
      .find_map(|(name, value)| (name == REASONING_EFFORT).then_some(value));
    CodexInvocation {
      executable: connection.executable(),
      model: model.as_str(),
      reasoning_effort: effort,
      access: *repository,
      worktree: self.worktree,
      platform: Platform::current(),
    }
    .command(prompt, std::env::vars_os())
    .map_err(|error| problem(error.to_string()))
  }

  fn not_started(&mut self, invocation: &mut Invocation, problem: RunnerProblem) -> Result<StepEnd, StepError> {
    tracing::warn!(run = %self.run.id(), ?problem, "the agent did not start");
    self.record(invocation, &[(Timestamp::now(), AgentEvent::RunnerProblem { problem })])?;
    self.finish(invocation, InvocationOutcome::NotStarted)?;
    Ok(StepEnd::Failed)
  }

  /// Follows the CLI until it exits, recording its output as it arrives. Ends the CLI when the
  /// runner is asked to stop.
  async fn stream(
    &mut self,
    invocation: &mut Invocation,
    mut process: Supervised,
  ) -> Result<(InvocationOutcome, Option<InterruptCause>), StepError> {
    let mut tracker = InvocationTracker::default();
    let mut pending = Vec::new();
    let mut flush = tokio::time::interval(FLUSH_INTERVAL);
    let mut stopped = None;
    let mut watching = true;
    loop {
      tokio::select! {
        event = process.next() => match event {
          Some(ProcessEvent::Output { stream, line }) => {
            let event = parse_codex_line(stream, &line);
            tracker.observe(&event);
            pending.push((Timestamp::now(), event));
            if pending.len() >= FLUSH_BATCH {
              self.flush(invocation, &tracker, &mut pending)?;
            }
          }
          Some(ProcessEvent::Exited(exit)) => {
            self.flush(invocation, &tracker, &mut pending)?;
            return Ok((outcome(&tracker, &exit, stopped), stopped));
          }
          None => {
            self.flush(invocation, &tracker, &mut pending)?;
            return Ok((InvocationOutcome::ExitFailed { code: None }, stopped));
          }
        },
        _ = flush.tick() => self.flush(invocation, &tracker, &mut pending)?,
        changed = self.stop.changed(), if watching => {
          watching = changed.is_ok();
          if let Some(cause) = *self.stop.borrow() {
            stopped = Some(cause);
            watching = false;
            process.terminate();
          }
        }
      }
    }
  }

  fn flush(
    &mut self,
    invocation: &mut Invocation,
    tracker: &InvocationTracker,
    pending: &mut Vec<(Timestamp, AgentEvent)>,
  ) -> Result<(), StepError> {
    if pending.is_empty() {
      return Ok(());
    }
    let session = tracker.session().map(str::to_owned);
    if invocation.session != session || invocation.usage != tracker.usage() {
      invocation.session = session;
      invocation.usage = tracker.usage();
      self.db.save_invocation(invocation)?;
    }
    self.record(invocation, pending)?;
    pending.clear();
    Ok(())
  }

  fn record(&mut self, invocation: &Invocation, events: &[(Timestamp, AgentEvent)]) -> Result<(), StepError> {
    let sequence = self.db.append_activity(invocation, events)?;
    self.notifier.send(Event::AgentActivity {
      run: self.run.id(),
      sequence,
    });
    Ok(())
  }

  fn finish(&mut self, invocation: &mut Invocation, outcome: InvocationOutcome) -> Result<(), StepError> {
    invocation.ended_at = Some(Timestamp::now());
    invocation.outcome = Some(outcome);
    Ok(self.db.save_invocation(invocation)?)
  }

  /// Commits everything the step changed in the worktree, with the repository's git identity.
  fn commit(&self, execution: StepExecutionId) -> Result<Option<Revision>, WorktreeError> {
    let git = Git::new(self.worktree);
    if git
      .stdout(["status", "--porcelain", "--untracked-files=all"])?
      .is_empty()
    {
      return Ok(None);
    }
    let step = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().body().step(execution.step()))
      .map_or_else(String::new, |step| step.name.as_str().to_owned());
    let message = format!("{}: {step} (run {})", self.task.key(), self.run.number());
    git.stdout(["add", "--all"])?;
    git.stdout(["commit", "--quiet", "-m", &message])?;
    let head = git.stdout(["rev-parse", "HEAD"])?;
    Revision::new(&head)
      .map(Some)
      .map_err(|_| WorktreeError::UnexpectedRevision(head))
  }
}

fn outcome(tracker: &InvocationTracker, exit: &ProcessExit, stopped: Option<InterruptCause>) -> InvocationOutcome {
  match (exit, stopped) {
    (ProcessExit::Terminated, _) | (_, Some(_)) => InvocationOutcome::Terminated,
    (ProcessExit::Exited { code }, None) => tracker.outcome(*code),
    (ProcessExit::WaitFailed { .. }, None) => InvocationOutcome::ExitFailed { code: None },
  }
}
