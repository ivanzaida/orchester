//! Executing runs: creating a run for a queued task, its worktree, and driving its agent steps
//! one after another on the Codex CLI. Every state change is recorded in the project database
//! first; the desktop is only told *that* something changed ([`Event::RunChanged`],
//! [`Event::AgentActivity`]) and reads the database.
//!
//! This version executes pipelines of agent steps sequentially, in dependency order, without
//! approval gates or tool steps (the preflight refuses those).

mod prepare;
mod run;
mod step;

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  run::InterruptCause,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;
use parking_lot::Mutex;
use tokio::{
  sync::{mpsc, watch},
  task::JoinHandle,
};

use self::run::RunTask;

/// How long stopping waits for each run to end its agent and record the interruption.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// Where the runner's change notifications go: the connected desktop.
#[derive(Clone)]
pub struct Notifier(mpsc::UnboundedSender<Event>);

impl Notifier {
  pub fn new(events: mpsc::UnboundedSender<Event>) -> Self {
    Self(events)
  }

  fn send(&self, event: Event) {
    // Without a desktop listening nobody needs the notification; the database has the change.
    if self.0.send(event).is_err() {
      tracing::debug!("no desktop is listening for notifications");
    }
  }
}

struct ActiveRun {
  stop: watch::Sender<Option<InterruptCause>>,
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
  active: Mutex<HashMap<RunId, ActiveRun>>,
}

impl Scheduler {
  pub fn new(project_root: &Path, notifier: Notifier) -> Self {
    Self {
      inner: Arc::new(Inner {
        project_root: project_root.to_path_buf(),
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
    let mut db = ProjectDb::open(root).map_err(|error| failed("cannot open the project database", &error))?;
    let prepared = prepare::prepare(&mut db, root, key, task)?;
    let run = prepared.run.id();
    self.inner.notifier.send(Event::RunChanged {
      run,
      task: prepared.run.task(),
    });
    if !prepared.execute {
      return Ok(run);
    }
    let mut active = self.inner.active.lock();
    if active.get(&run).is_some_and(|active| !active.task.is_finished()) {
      return Ok(run);
    }
    let (stop, stopped) = watch::channel(None);
    let task = RunTask::new(db, prepared, self.inner.notifier.clone(), stopped);
    let task = tokio::spawn(task.drive());
    active.insert(run, ActiveRun { stop, task });
    Ok(run)
  }

  /// Ends the agents of every active run and records the runs as interrupted by `cause`,
  /// waiting at most [`STOP_TIMEOUT`] for each.
  pub async fn stop_all(&self, cause: InterruptCause) {
    let active: Vec<(RunId, ActiveRun)> = self.inner.active.lock().drain().collect();
    for (run, active) in active {
      // A run that already ended has dropped its receiver; there is nothing to stop.
      let _ = active.stop.send(Some(cause));
      let mut task = active.task;
      if tokio::time::timeout(STOP_TIMEOUT, &mut task).await.is_err() {
        tracing::warn!(%run, "the run did not stop within {STOP_TIMEOUT:?}; ending it");
        task.abort();
      }
    }
  }
}

fn failed(what: &str, error: &dyn std::error::Error) -> Rejection {
  tracing::error!("{what}: {error}");
  Rejection::Failed {
    detail: format!("{what}: {error}"),
  }
}
