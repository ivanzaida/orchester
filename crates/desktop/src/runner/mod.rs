//! The runner of the open project (`docs/architecture/storage.md`, "Processes"): one
//! `orchester-runner` per open project, started when the project opens, reconnected when it
//! drops and stopped when the project or the application closes.
//!
//! Windows only for now: the IPC client (`orchester_ipc::RunnerClient`, a named pipe and a job
//! object) exists only on Windows.

mod connection;
mod report;
pub mod state;

use std::{
  io,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::{Duration, SystemTime},
};

use parking_lot::Mutex;
use tokio::{runtime::Handle, sync::watch};

pub use self::report::RunnerStatus;
use self::{connection::Connection, report::Reporter, state::RunnerEvent};
use crate::activity::Activity;

/// How long a close without active work waits for the runner to stop and exit before
/// terminating it.
pub const QUIET_STOP_TIMEOUT: Duration = Duration::from_secs(6);

/// How asking the runner to stop ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownOutcome {
  /// No runner was running.
  NoRunner,
  /// The runner confirmed that it stopped.
  Stopped,
  /// The runner did not confirm in time or went away first; it was terminated.
  Unanswered,
}

/// A run with an agent at work that closing the application would interrupt, as listed by the
/// close confirmation (`clVjO` rows in `um9B1`). Every value is runtime data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveWork {
  project: Arc<str>,
  task_key: Arc<str>,
  task_title: Arc<str>,
  step: Arc<str>,
  agent: Arc<str>,
  model: Arc<str>,
}

/// The fields of [`ActiveWork`], named so they cannot be swapped by position.
pub struct ActiveWorkFields<'a> {
  pub project: &'a str,
  pub task_key: &'a str,
  pub task_title: &'a str,
  pub step: &'a str,
  pub agent: &'a str,
  pub model: &'a str,
}

impl ActiveWork {
  pub fn new(fields: &ActiveWorkFields<'_>) -> Self {
    Self {
      project: fields.project.into(),
      task_key: fields.task_key.into(),
      task_title: fields.task_title.into(),
      step: fields.step.into(),
      agent: fields.agent.into(),
      model: fields.model.into(),
    }
  }

  pub fn project(&self) -> &str {
    &self.project
  }

  pub fn task_key(&self) -> &str {
    &self.task_key
  }

  pub fn task_title(&self) -> &str {
    &self.task_title
  }

  pub fn step(&self) -> &str {
    &self.step
  }

  pub fn agent(&self) -> &str {
    &self.agent
  }

  /// Provider and model, e.g. "Codex · Sol".
  pub fn model(&self) -> &str {
    &self.model
  }
}

impl lurq::app::component::DevtoolsInspectable for ActiveWork {}

/// Starts and stops the runner of the open project and publishes its status.
#[derive(Clone)]
pub struct Runners {
  inner: Arc<Inner>,
}

struct Inner {
  runner_exe: PathBuf,
  runtime: Handle,
  status: watch::Sender<RunnerStatus>,
  /// Where the runner's events (a reconnect) are recorded for the status bar.
  activity: Activity,
  current: Mutex<Option<Connection>>,
  /// Set once the application closes; no runner starts after that.
  closing: AtomicBool,
}

impl PartialEq for Runners {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl lurq::app::component::DevtoolsInspectable for Runners {}

impl Runners {
  pub fn new(runner_exe: PathBuf, runtime: Handle, activity: Activity) -> Self {
    Self {
      inner: Arc::new(Inner {
        runner_exe,
        runtime,
        status: watch::Sender::new(RunnerStatus::none(0)),
        activity,
        current: Mutex::new(None),
        closing: AtomicBool::new(false),
      }),
    }
  }

  /// Makes `project` the project whose runner runs: starts its runner unless it already has
  /// one, and stops the runner of the previous project. `None` only stops.
  pub fn show(&self, project: Option<&Path>) {
    if self.inner.closing.load(Ordering::Acquire) {
      return;
    }
    let mut current = self.inner.current.lock();
    let status = self.inner.status.borrow().clone();
    if status.project_root() == project {
      return;
    }
    let previous = current.take();
    let generation = status.generation() + 1;
    let Some(project) = project else {
      self.inner.status.send_replace(RunnerStatus::none(generation));
      if let Some(previous) = previous {
        self.inner.runtime.spawn(previous.stop(Some(QUIET_STOP_TIMEOUT)));
      }
      return;
    };
    tracing::info!(project = %project.display(), "starting the runner");
    self
      .inner
      .status
      .send_replace(RunnerStatus::starting(generation, project.to_path_buf()));
    *current = Some(self.connect(generation, project, previous));
  }

  /// Starts the runner again after it failed or stopped; the reconnect attempts start over.
  pub fn restart(&self) {
    if self.inner.closing.load(Ordering::Acquire) {
      return;
    }
    let mut current = self.inner.current.lock();
    let restarted = self.inner.status.send_if_modified(|status| {
      status.project_root().is_some() && status.state().can_restart() && status.apply(RunnerEvent::RestartRequested)
    });
    if !restarted {
      return;
    }
    let status = self.inner.status.borrow().clone();
    let Some(project) = status.project_root() else {
      return;
    };
    tracing::info!(project = %project.display(), "restarting the runner");
    // The previous task has ended; its handle only needs to go.
    drop(current.take());
    *current = Some(self.connect(status.generation(), project, None));
  }

  fn connect(&self, generation: u64, project: &Path, previous: Option<Connection>) -> Connection {
    Connection::spawn(
      &self.inner.runtime,
      self.inner.runner_exe.clone(),
      project.to_path_buf(),
      Reporter::new(generation, self.inner.status.clone(), self.inner.activity.clone()),
      previous,
    )
  }

  /// Opens today's runner log of the open project with its default program, or the logs
  /// folder when the runner has not written a log today.
  pub fn open_log(&self) {
    let Some(project) = self.status().project_root().map(Path::to_path_buf) else {
      return;
    };
    self.inner.runtime.spawn_blocking(move || {
      if let Err(error) = open_log(&project) {
        tracing::error!(project = %project.display(), %error, "cannot open the runner log");
      }
    });
  }

  pub fn subscribe(&self) -> watch::Receiver<RunnerStatus> {
    self.inner.status.subscribe()
  }

  pub fn status(&self) -> RunnerStatus {
    self.inner.status.borrow().clone()
  }

  /// Work that closing would interrupt. Runs do not exist yet and the runner protocol reports
  /// none, so nothing is ever active; this is the one place that will ask the runner.
  pub fn active_work(&self) -> Vec<ActiveWork> {
    Vec::new()
  }

  /// Stops the runner for the application to close: at checkpoints, waiting at most
  /// `deadline` when given. No runner starts afterwards.
  pub async fn shutdown(&self, deadline: Option<Duration>) -> ShutdownOutcome {
    self.inner.closing.store(true, Ordering::Release);
    let current = self.inner.current.lock().take();
    match current {
      Some(connection) => connection.stop(deadline).await,
      None => ShutdownOutcome::NoRunner,
    }
  }

  /// "Quit now": terminates the runner and its agents without a checkpoint.
  pub fn terminate(&self) {
    self.inner.closing.store(true, Ordering::Release);
    drop(self.inner.current.lock().take());
  }
}

fn open_log(project: &Path) -> io::Result<()> {
  let log = orchester_ipc::runner_log_path(project, SystemTime::now());
  let target = if log.is_file() {
    log
  } else {
    orchester_ipc::runner_log_dir(project)
  };
  crate::external::open_path(&target)
}
