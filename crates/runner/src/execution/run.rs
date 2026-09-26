//! Driving one run: its steps one after another in dependency order until the run succeeds or
//! fails, or settles: paused at a checkpoint, stopped at a limit, cancelled or interrupted. Each
//! transition is appended to the run's log and announced. Requests for the run (pause, cancel,
//! the runner stopping) arrive as [`Control`]s, between steps and while an agent works.

use std::{path::PathBuf, time::Duration};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::StepExecutionId,
  project::Project,
  run::{InterruptCause, Invocation, Run, RunError, RunEvent, RunLimit, RunStatus, StepStatus, active_time},
  store::ActivityStore,
  task::Task,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;
use tokio::sync::mpsc;

use super::{
  Notifier, checkpoint,
  control::{Ack, Control, answer},
  prepare::Prepared,
  record::{RecordError, record},
  step::StepEnd,
};

#[derive(Debug, Error)]
pub(super) enum ExecutionError {
  #[error(transparent)]
  Record(#[from] RecordError),
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("step execution {0} is not an agent step of the run")]
  NotAnAgentStep(StepExecutionId),
}

/// What the task of a run is given besides the run: where it announces changes and where its
/// requests arrive.
pub(super) struct TaskContext {
  pub(super) notifier: Notifier,
  pub(super) controls: mpsc::UnboundedReceiver<Control>,
}

/// Requests received and not carried out yet; they take effect once the agent has ended.
#[derive(Default)]
pub(super) struct Pending {
  pub(super) cancel: Option<Ack>,
  pub(super) interrupt: Option<InterruptCause>,
}

pub(super) struct RunTask {
  pub(super) db: ProjectDb,
  pub(super) run: Run,
  pub(super) task: Task,
  pub(super) project: Project,
  pub(super) worktree: PathBuf,
  pub(super) notifier: Notifier,
  pub(super) controls: mpsc::UnboundedReceiver<Control>,
  pub(super) pending: Pending,
}

impl RunTask {
  pub(super) fn new(db: ProjectDb, prepared: Prepared, context: TaskContext) -> Self {
    let project = db.project().clone();
    Self {
      db,
      run: prepared.run,
      task: prepared.task,
      project,
      worktree: prepared.worktree,
      notifier: context.notifier,
      controls: context.controls,
      pending: Pending::default(),
    }
  }

  /// Executes the run until it ends or settles. A failure of the runner itself (the database, a
  /// rejected transition) leaves the run as last recorded and is logged; its steps are not
  /// retried.
  pub(super) async fn drive(mut self) {
    let run = self.run.id();
    tracing::info!(%run, "run executing");
    match self.steps().await {
      Ok(()) => tracing::info!(%run, status = ?self.run.status(), "run settled"),
      Err(error) => {
        tracing::error!(%run, %error, "the run could not continue");
        if let Some(ack) = self.pending.cancel.take() {
          answer(
            ack,
            Err(Rejection::Failed {
              detail: error.to_string(),
            }),
          );
        }
      }
    }
  }

  async fn steps(&mut self) -> Result<(), ExecutionError> {
    loop {
      while let Ok(control) = self.controls.try_recv() {
        self.control(control)?;
      }
      if self.settle()? {
        return Ok(());
      }
      let Some(execution) = self.next_execution() else {
        return Ok(());
      };
      let resume = self.begin_step(execution)?;
      let time_left = self.time_left()?;
      let end = self.execute(execution, resume, time_left).await?;
      match end {
        StepEnd::Passed { checkpoint } => {
          if let Some(revision) = checkpoint {
            self.transition(|run, now| run.record_checkpoint(execution, revision, now))?;
          }
          self.transition(|run, now| run.complete_step(execution, now))?;
        }
        StepEnd::Failed => self.transition(|run, now| run.fail_step(execution, now))?,
        // The request that stopped the agent is carried out at the top of the loop.
        StepEnd::Stopped => {}
        StepEnd::Limit(limit) => return self.stop_at_limit(limit),
      }
    }
  }

  /// Carries out what must happen before the next step: a cancel or interruption, a requested
  /// pause, a reached limit. `true` when the run no longer executes.
  fn settle(&mut self) -> Result<bool, ExecutionError> {
    if let Some(ack) = self.pending.cancel.take() {
      let cancelled = self.transition(|run, now| run.cancel(now));
      let result = match &cancelled {
        Ok(()) => Ok(()),
        Err(error) => Err(Rejection::Failed {
          detail: error.to_string(),
        }),
      };
      answer(ack, result);
      return cancelled.map(|()| true);
    }
    if self.run.status() != &RunStatus::Running {
      return Ok(true);
    }
    if let Some(cause) = self.pending.interrupt {
      self.transition(|run, now| run.interrupt(cause, now))?;
      return Ok(true);
    }
    if self.run.pause_requested() {
      self.transition(|run, now| run.pause(now))?;
      return Ok(true);
    }
    if self.run.token_limit_reached() {
      self.stop_at_limit(RunLimit::Tokens)?;
      return Ok(true);
    }
    if self.time_left()?.is_zero() {
      self.stop_at_limit(RunLimit::Time)?;
      return Ok(true);
    }
    Ok(false)
  }

  /// Handles a request. A pause is recorded at once and waits for the next checkpoint; a cancel
  /// or the runner stopping ends the agent. `true` when the agent must be ended.
  pub(super) fn control(&mut self, control: Control) -> Result<bool, ExecutionError> {
    match control {
      Control::Pause(ack) => {
        let requested = self.transition(|run, now| run.request_pause(now));
        let result = match &requested {
          Ok(()) => Ok(()),
          Err(error) => Err(Rejection::Failed {
            detail: error.to_string(),
          }),
        };
        answer(ack, result);
        requested.map(|()| false)
      }
      Control::Cancel(ack) => {
        self.pending.cancel = Some(ack);
        Ok(true)
      }
      Control::Interrupt(cause) => {
        self.pending.interrupt = Some(cause);
        Ok(true)
      }
    }
  }

  /// The run reached `limit`: what its agent changed is committed as a checkpoint, then the run
  /// waits as "Stopped at limit". A failed commit is reported in the transcript; the run still
  /// stops, and its uncommitted changes stay in the worktree.
  fn stop_at_limit(&mut self, limit: RunLimit) -> Result<(), ExecutionError> {
    if let Some(execution) = self.running_execution() {
      let message = format!(
        "{}: work in progress at the {} limit (run {})",
        self.task.key(),
        match limit {
          RunLimit::Tokens => "token",
          RunLimit::Time => "time",
        },
        self.run.number()
      );
      match checkpoint::commit(&self.worktree, &message) {
        Ok(Some(revision)) => self.transition(|run, now| run.record_checkpoint(execution, revision, now))?,
        Ok(None) => {}
        Err(error) => self.report_checkpoint_failure(execution, &error.to_string())?,
      }
    }
    self.transition(|run, now| run.stop_at_limit(limit, now))
  }

  fn report_checkpoint_failure(&mut self, execution: StepExecutionId, detail: &str) -> Result<(), ExecutionError> {
    tracing::warn!(run = %self.run.id(), detail, "the work at the limit could not be committed");
    let Some(invocation) = self.latest_invocation(execution)? else {
      return Ok(());
    };
    let problem = RunnerProblem::CheckpointFailed {
      detail: detail.to_owned(),
    };
    self.record_activity(
      &invocation,
      &[(Timestamp::now(), AgentEvent::RunnerProblem { problem })],
    )
  }

  /// Starts `execution` if it has not started. One that was stopped mid-step continues: the
  /// session of its last invocation, if the CLI reported one, is resumed.
  fn begin_step(&mut self, execution: StepExecutionId) -> Result<Option<String>, ExecutionError> {
    let status = self.run.execution(execution).map(|execution| execution.status());
    if status == Some(StepStatus::Pending) {
      self.transition(|run, now| run.start_step(execution, now))?;
      return Ok(None);
    }
    Ok(
      self
        .latest_invocation(execution)?
        .and_then(|invocation| invocation.session),
    )
  }

  /// Active agent time left before the time limit.
  fn time_left(&self) -> Result<Duration, ExecutionError> {
    let invocations = self.db.invocations(self.run.id())?;
    let active = active_time(&invocations, Timestamp::now());
    Ok(self.run.max_duration().saturating_sub(active))
  }

  fn latest_invocation(&self, execution: StepExecutionId) -> Result<Option<Invocation>, ExecutionError> {
    Ok(
      self
        .db
        .invocations(self.run.id())?
        .into_iter()
        .rev()
        .find(|invocation| invocation.execution == execution),
    )
  }

  fn running_execution(&self) -> Option<StepExecutionId> {
    self.run.current_attempt().and_then(|attempt| {
      attempt
        .executions()
        .iter()
        .find(|execution| execution.status() == StepStatus::Running)
        .map(|execution| execution.id())
    })
  }

  /// The execution to work on: one stopped mid-step, else the first pending execution of the
  /// current attempt, in pipeline order, whose dependencies have passed.
  fn next_execution(&self) -> Option<StepExecutionId> {
    if let Some(running) = self.running_execution() {
      return Some(running);
    }
    let attempt = self.run.current_attempt()?;
    let body = self.run.snapshot().body();
    attempt
      .executions()
      .iter()
      .filter(|execution| execution.status() == StepStatus::Pending)
      .find(|execution| {
        body.step(execution.step()).is_some_and(|step| {
          step.depends_on.iter().all(|dependency| {
            self
              .run
              .latest_execution(dependency)
              .is_some_and(|done| done.status() == StepStatus::Passed)
          })
        })
      })
      .map(|execution| execution.id())
  }

  /// Applies a transition, stores its events with the new state and announces the change.
  pub(super) fn transition(
    &mut self,
    transition: impl FnOnce(&mut Run, Timestamp) -> Result<Vec<RunEvent>, RunError>,
  ) -> Result<(), ExecutionError> {
    Ok(record(&mut self.db, &self.notifier, &mut self.run, transition)?)
  }

  /// Appends transcript entries of `invocation` and announces them.
  pub(super) fn record_activity(
    &mut self,
    invocation: &Invocation,
    events: &[(Timestamp, AgentEvent)],
  ) -> Result<(), ExecutionError> {
    let sequence = self.db.append_activity(invocation, events)?;
    self.notifier.send(Event::AgentActivity {
      run: self.run.id(),
      sequence,
    });
    Ok(())
  }
}
