//! The runner of the open project (`docs/architecture/storage.md`, "Processes"): one
//! `orchester-runner` per open project, started when the project opens, reconnected when it
//! drops and stopped when the project or the application closes.
//!
//! Windows only for now: the IPC client (`orchester_ipc::RunnerClient`, a named pipe and a job
//! object) exists only on Windows.

mod connection;
mod report;
mod requests;
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

use orchester_core::id::{IdempotencyKey, RunId, TaskId};
use orchester_ipc::{Command, Rejection, Reply};
use parking_lot::Mutex;
use tokio::{runtime::Handle, sync::watch};

use self::{connection::Connection, report::Reporter, state::RunnerEvent};
pub use self::{report::RunnerStatus, requests::RequestError};
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

/// Why the runner did not do what a request about a run asked.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RunRequestError {
  #[error(transparent)]
  Request(#[from] RequestError),
  #[error("the runner refused the request: {0:?}")]
  Rejected(Rejection),
}

/// Starts and stops the runner of the open project and publishes its status.
#[derive(Clone)]
pub struct Runners {
  inner: Arc<Inner>,
}

struct Inner {
  runner_exe: PathBuf,
  runtime: Handle,
  status: watch::Sender<RunnerStatus>,
  /// Counts the runner's run change notifications; screens showing runs read them again on
  /// every change.
  runs: watch::Sender<u64>,
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
        runs: watch::Sender::new(0),
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
      Reporter::new(
        generation,
        self.inner.status.clone(),
        self.inner.activity.clone(),
        self.inner.runs.clone(),
      ),
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

  /// Changes whenever the runner reports that a run or its transcript changed.
  pub fn run_changes(&self) -> watch::Receiver<u64> {
    self.inner.runs.subscribe()
  }

  /// Asks the open project's runner to start a run of queued task `task`. Idempotent by `key`:
  /// retrying after a lost reply answers with the run the first request created.
  pub async fn start_run(&self, key: IdempotencyKey, task: TaskId) -> Result<RunId, RunRequestError> {
    match self.ask(Command::StartRun { key, task }).await? {
      Reply::RunStarted { run } => Ok(run),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to StartRun");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Asks the open project's runner to pause, resume or cancel a run, or to raise its limits
  /// (`command`); returns once the runner has carried it out.
  pub async fn control_run(&self, command: Command) -> Result<(), RunRequestError> {
    match self.ask(command).await? {
      Reply::Accepted { .. } => Ok(()),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to a run control request");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  async fn ask(&self, command: Command) -> Result<Reply, RequestError> {
    let answer = match self.inner.current.lock().as_ref() {
      Some(connection) => connection.request(command),
      None => return Err(RequestError::NotConnected),
    };
    // The task ended without answering, e.g. when the project closed meanwhile.
    answer.await.unwrap_or(Err(RequestError::Lost))
  }

  /// Work that closing would interrupt. The runner protocol does not list active work yet, so
  /// nothing is listed and closing interrupts running agents (their runs are recorded as
  /// interrupted); this is the one place that will ask the runner.
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
