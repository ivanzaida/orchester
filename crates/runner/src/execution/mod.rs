//! Executing runs: creating a run for a queued task, its worktree, and driving its agent steps on
//! the Codex CLI; pausing, resuming, cancelling and continuing a run stopped
//! at a limit. Every state change is recorded in the project database first; the desktop is only
//! told *that* something changed ([`Event::RunChanged`], [`Event::AgentActivity`]) and reads the
//! database.
//!
//! This version executes pipeline graphs of agent steps: every step whose dependencies have passed
//! runs, beside the others (see `workspace` for where each works), with failure routes and retries,
//! step budgets and the outputs of earlier steps in later prompts. Approval gates and tool steps
//! are refused by the preflight. Every agent at work takes one of the desktop's agent slots, which
//! all runs share; the project's daily token budget holds new and continued work back once today's
//! runs used it.

mod attach;
mod board;
mod checkpoint;
mod control;
mod halt;
mod idle;
mod prepare;
mod record;
mod run;
mod step;
mod work;
mod workspace;

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  run::{InterruptCause, RaisedLimits},
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;
use parking_lot::Mutex;
use tokio::{
  sync::{mpsc, oneshot},
  task::JoinHandle,
};

pub use self::attach::Attached;
use self::{
  board::Board,
  control::Control,
  prepare::Prepared,
  run::{RunTask, TaskContext},
};
pub use crate::notify::Notifier;

/// How long stopping waits for each run to end its agent and record the interruption.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

struct ActiveRun {
  control: mpsc::UnboundedSender<Control>,
  task: JoinHandle<()>,
}

/// The runs this runner executes.
#[derive(Clone)]
pub struct Scheduler {
  inner: Arc<Inner>,
}

struct Inner {
  project_root: PathBuf,
  notifier: Notifier,
  board: Arc<Board>,
  active: Mutex<HashMap<RunId, ActiveRun>>,
}

impl Scheduler {
  pub fn new(project_root: &Path, notifier: Notifier) -> Self {
    Self {
      inner: Arc::new(Inner {
        project_root: project_root.to_path_buf(),
        board: Arc::new(Board::new(notifier.clone())),
        notifier,
        active: Mutex::new(HashMap::new()),
      }),
    }
  }

  /// Runs being executed right now.
  pub fn active_runs(&self) -> Vec<RunId> {
    let mut active = self.inner.active.lock();
    active.retain(|_, run| !run.task.is_finished());
    active.keys().copied().collect()
  }

  /// Creates and starts the run of queued task `task`, or finds the one an earlier request with
  /// `key` created. Must be called within a Tokio runtime; returns once the run is recorded as
  /// started and its first step is scheduled.
  pub fn start_run(&self, key: IdempotencyKey, task: TaskId) -> Result<RunId, Rejection> {
    let root = &self.inner.project_root;
    let mut db = self.open()?;
    self.admit(&db)?;
    let prepared = prepare::prepare(&mut db, root, key, task)?;
    let run = prepared.run.id();
    self.inner.notifier.send(Event::RunChanged {
      run,
      task: prepared.run.task(),
    });
    if prepared.execute {
      self.spawn(db, prepared);
    }
    Ok(run)
  }

  /// Pauses `run` at its next checkpoint. Returns once the request is recorded; the run is
  /// paused when its active agent invocation has ended and its work is committed.
  pub async fn pause_run(&self, run: RunId) -> Result<(), Rejection> {
    match self.ask(run, Control::Pause) {
      Some(answer) => match answer.await {
        Ok(result) => result,
        Err(_) => idle::pause(&mut self.open()?, run),
      },
      None => idle::pause(&mut self.open()?, run),
    }
  }

  /// Cancels `run`: ends its agent, keeps its worktree and branch. Returns once the run is
  /// recorded as cancelled.
  pub async fn cancel_run(&self, run: RunId) -> Result<(), Rejection> {
    let asked = match self.ask(run, Control::Cancel) {
      Some(answer) => answer.await.ok(),
      None => None,
    };
    // Without an answer the run's task ended before it read the request: the run is idle now.
    if let Some(result) = asked {
      return result;
    }
    let cancelled = idle::cancel(&mut self.open()?, run)?;
    self.inner.notifier.send(Event::RunChanged {
      run,
      task: cancelled.task(),
    });
    Ok(())
  }

  /// Continues paused run `run`; a step it stopped in continues the agent's CLI session.
  pub fn resume_run(&self, run: RunId) -> Result<(), Rejection> {
    self.continue_idle(run, |db, root| idle::resume(db, root, run))
  }

  /// Raises the limits of `run`, stopped at a limit, and continues it.
  pub fn raise_limit(&self, run: RunId, limits: RaisedLimits) -> Result<(), Rejection> {
    self.continue_idle(run, |db, root| idle::raise(db, root, run, limits))
  }

  /// "Wait for agents to save": starts no new work, asks every active run to pause at its next
  /// checkpoint and returns once each has settled. The work summary shows each run saving, then
  /// saved.
  pub async fn finish_at_checkpoints(&self) {
    self.inner.board.stop_at_checkpoints();
    let active: Vec<RunId> = self.active_runs();
    let mut settling = Vec::new();
    for run in active {
      match self.ask(run, Control::Pause) {
        Some(answer) => settling.push((run, answer)),
        None => tracing::debug!(%run, "the run settled before it was asked to pause"),
      }
    }
    for (run, answer) in settling {
      match answer.await {
        Ok(Ok(())) => {}
        Ok(Err(rejection)) => tracing::warn!(%run, ?rejection, "the run could not be asked to pause"),
        Err(_) => tracing::debug!(%run, "the run settled before it read the pause"),
      }
    }
    let tasks: Vec<(RunId, ActiveRun)> = self.inner.active.lock().drain().collect();
    for (run, active) in tasks {
      if let Err(error) = active.task.await {
        tracing::warn!(%run, %error, "the run's task ended abnormally");
      }
    }
  }

  /// Ends the agents of every active run and records the runs as interrupted by `cause`,
  /// waiting at most [`STOP_TIMEOUT`] for each.
  pub async fn stop_all(&self, cause: InterruptCause) {
    let active: Vec<(RunId, ActiveRun)> = self.inner.active.lock().drain().collect();
    for (run, active) in active {
      // A run that already ended has dropped its receiver; there is nothing to stop.
      if active.control.send(Control::Interrupt(cause)).is_err() {
        tracing::debug!(%run, "the run had already ended");
      }
      let mut task = active.task;
      if tokio::time::timeout(STOP_TIMEOUT, &mut task).await.is_err() {
        tracing::warn!(%run, "the run did not stop within {STOP_TIMEOUT:?}; ending it");
        task.abort();
      }
    }
  }

  fn open(&self) -> Result<ProjectDb, Rejection> {
    ProjectDb::open(&self.inner.project_root).map_err(|error| failed("cannot open the project database", &error))
  }

  /// Hands `control` to the task executing `run`, if one does; `None` when the run is idle.
  fn ask<T>(&self, run: RunId, control: impl FnOnce(oneshot::Sender<T>) -> Control) -> Option<oneshot::Receiver<T>> {
    let active = self.inner.active.lock();
    let active = active.get(&run).filter(|active| !active.task.is_finished())?;
    let (answer, answered) = oneshot::channel();
    active.control.send(control(answer)).ok()?;
    Some(answered)
  }

  /// Whether new or continued work may start: the runner is not stopping, an agent slot is
  /// free and today's token budget has room.
  fn admit(&self, db: &ProjectDb) -> Result<(), Rejection> {
    if self.inner.board.is_stopping() {
      return Err(Rejection::ShuttingDown);
    }
    // Each executing run has at least one agent at work or about to be, and its parallel steps
    // take further slots while they are free.
    if let Some(slots) = self.inner.board.slots()
      && (self.active_runs().len() >= slots.get() as usize || !self.inner.board.has_free_slot())
    {
      return Err(Rejection::NoAgentSlot { slots });
    }
    idle::check_daily_budget(db)
  }

  /// Applies `transition` to idle run `run` and executes it again.
  fn continue_idle(
    &self,
    run: RunId,
    transition: impl FnOnce(&mut ProjectDb, &Path) -> Result<Prepared, Rejection>,
  ) -> Result<(), Rejection> {
    if self.is_active(run) {
      return Err(Rejection::RunNotPaused);
    }
    let mut db = self.open()?;
    self.admit(&db)?;
    let prepared = transition(&mut db, &self.inner.project_root)?;
    self.inner.notifier.send(Event::RunChanged {
      run,
      task: prepared.run.task(),
    });
    self.spawn(db, prepared);
    Ok(())
  }

  fn is_active(&self, run: RunId) -> bool {
    self
      .inner
      .active
      .lock()
      .get(&run)
      .is_some_and(|active| !active.task.is_finished())
  }

  fn spawn(&self, db: ProjectDb, prepared: Prepared) {
    let mut active = self.inner.active.lock();
    let run = prepared.run.id();
    if active.get(&run).is_some_and(|active| !active.task.is_finished()) {
      return;
    }
    let (control, controls) = mpsc::unbounded_channel();
    let context = TaskContext {
      root: self.inner.project_root.clone(),
      notifier: self.inner.notifier.clone(),
      board: self.inner.board.clone(),
      controls,
    };
    let task = tokio::spawn(RunTask::new(db, prepared, context).drive());
    active.insert(run, ActiveRun { control, task });
  }
}

fn failed(what: &str, error: &dyn std::error::Error) -> Rejection {
  tracing::error!("{what}: {error}");
  Rejection::Failed {
    detail: format!("{what}: {error}"),
  }
}
