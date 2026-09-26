//! Driving one started run: its steps one after another in dependency order until the run
//! succeeds, fails or is stopped. Each transition is appended to the run's log and announced.

use std::path::PathBuf;

use orchester_core::{
  id::StepExecutionId,
  project::Project,
  run::{InterruptCause, Run, RunError, RunEvent, StepStatus},
  store::RunStore,
  task::Task,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{ProjectDb, RunStoreError};
use thiserror::Error;
use tokio::sync::watch;

use super::{
  Notifier,
  prepare::Prepared,
  step::{StepEnd, StepError, StepRun},
};

#[derive(Debug, Error)]
pub(super) enum ExecutionError {
  #[error(transparent)]
  Run(#[from] RunError),
  #[error(transparent)]
  Store(#[from] RunStoreError),
  #[error(transparent)]
  Step(#[from] StepError),
}

pub(super) struct RunTask {
  db: ProjectDb,
  run: Run,
  task: Task,
  project: Project,
  worktree: PathBuf,
  notifier: Notifier,
  stop: watch::Receiver<Option<InterruptCause>>,
}

impl RunTask {
  pub(super) fn new(
    db: ProjectDb,
    prepared: Prepared,
    notifier: Notifier,
    stop: watch::Receiver<Option<InterruptCause>>,
  ) -> Self {
    let project = db.project().clone();
    Self {
      db,
      run: prepared.run,
      task: prepared.task,
      project,
      worktree: prepared.worktree,
      notifier,
      stop,
    }
  }

  /// Executes the run to its end. A failure of the runner itself (the database, a rejected
  /// transition) leaves the run as last recorded and is logged; its steps are not retried.
  pub(super) async fn drive(mut self) {
    let run = self.run.id();
    tracing::info!(%run, "run started");
    match self.steps().await {
      Ok(()) => tracing::info!(%run, status = ?self.run.status(), "run ended"),
      Err(error) => tracing::error!(%run, %error, "the run could not continue"),
    }
  }

  async fn steps(&mut self) -> Result<(), ExecutionError> {
    while let Some(execution) = self.next_execution() {
      let stopped = *self.stop.borrow();
      if let Some(cause) = stopped {
        return self.transition(|run, now| run.interrupt(cause, now));
      }
      self.transition(|run, now| run.start_step(execution, now))?;
      let end = StepRun {
        db: &mut self.db,
        run: &self.run,
        task: &self.task,
        project: &self.project,
        worktree: &self.worktree,
        notifier: &self.notifier,
        stop: &mut self.stop,
      }
      .execute(execution)
      .await?;
      match end {
        StepEnd::Passed { checkpoint } => {
          if let Some(revision) = checkpoint {
            self.transition(|run, now| run.record_checkpoint(execution, revision, now))?;
          }
          self.transition(|run, now| run.complete_step(execution, now))?;
        }
        StepEnd::Failed => self.transition(|run, now| run.fail_step(execution, now))?,
        StepEnd::Stopped(cause) => return self.transition(|run, now| run.interrupt(cause, now)),
      }
      if self.run.status().is_terminal() {
        break;
      }
    }
    Ok(())
  }

  /// The first pending execution of the current attempt, in pipeline order, whose dependencies
  /// have passed.
  fn next_execution(&self) -> Option<StepExecutionId> {
    if self.run.status().is_terminal() {
      return None;
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
  fn transition(
    &mut self,
    transition: impl FnOnce(&mut Run, Timestamp) -> Result<Vec<RunEvent>, RunError>,
  ) -> Result<(), ExecutionError> {
    let events = transition(&mut self.run, Timestamp::now())?;
    if events.is_empty() {
      return Ok(());
    }
    self.db.append_events(&self.run, &events)?;
    self.notifier.send(Event::RunChanged {
      run: self.run.id(),
      task: self.run.task(),
    });
    Ok(())
  }
}
