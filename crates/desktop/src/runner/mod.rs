//! The runners of the open projects (`docs/architecture/storage.md`, "Processes"): one
//! `orchester-runner` per project a window tab shows or whose runner has work, started when the
//! project opens in a tab, kept running in the background while another tab is active,
//! reconnected when it drops, stopped once no tab shows the project and it has no work (see
//! [`lifetime`]) and stopped when the application closes. The displayed project's runner status
//! is published for the screens.
//!
//! Windows only for now: the IPC client (`orchester_ipc::RunnerClient`, a named pipe and a job
//! object) exists only on Windows.

mod connection;
mod lifetime;
mod report;
mod requests;
pub mod state;
mod work;

use std::{
  collections::{HashMap, HashSet},
  io,
  num::NonZeroU32,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::{Duration, SystemTime},
};

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  integration::Leftovers,
};
use orchester_ipc::{AttachSession, Command, RefusedRun, Rejection, Reply, SessionEnd, WorkState};
use parking_lot::Mutex;
use tokio::{runtime::Handle, sync::watch};

use self::{
  connection::Connection,
  lifetime::Lingering,
  report::{Reporter, Shared},
  state::{RunnerEvent, RunnerState},
};
pub use self::{
  report::RunnerStatus,
  requests::RequestError,
  work::{ActiveWork, ActiveWorkFields, SaveState},
};
use crate::{activity::Activity, workspace::OpenProject};

/// How long a close without active work waits for the runner to stop and exit before
/// terminating it.
pub const QUIET_STOP_TIMEOUT: Duration = Duration::from_secs(6);

/// How many agents a project's runner lets work at once. Each run executes one agent step at a
/// time, so this is also how many runs execute at once. There is no setting for it yet (the
/// application `settings` in `docs/architecture/storage.md`); this is the default until there is.
pub const AGENT_SLOTS: NonZeroU32 = NonZeroU32::new(4).expect("four is above zero");

/// How long continuing interrupted runs waits for a project's runner to come online.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// How asking the runners to stop ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownOutcome {
  /// No runner was running.
  NoRunner,
  /// Every runner confirmed that it stopped.
  Stopped,
  /// A runner did not confirm in time or went away first; it was terminated.
  Unanswered,
}

/// Why the runner did not do what a request about a run asked.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RunRequestError {
  #[error(transparent)]
  Request(#[from] RequestError),
  #[error("the runner refused the request: {0:?}")]
  Rejected(Rejection),
}

/// The answer to continuing interrupted runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Continued {
  pub continued: Vec<RunId>,
  pub refused: Vec<RefusedRun>,
}

/// Starts and stops the projects' runners and publishes the displayed project's status.
#[derive(Clone)]
pub struct Runners {
  inner: Arc<Inner>,
}

struct Inner {
  runner_exe: PathBuf,
  runtime: Handle,
  /// How the previous session ended: each project's first runner attaches with it.
  previous_end: SessionEnd,
  shared: Arc<Shared>,
  projects: Mutex<HashMap<PathBuf, ProjectRunner>>,
  /// The projects the window's tabs show; the other runners linger until they have no work.
  tabbed: Mutex<HashSet<PathBuf>>,
  /// Set once the application closes; no runner starts after that.
  closing: AtomicBool,
}

/// One project's runner.
struct ProjectRunner {
  name: Arc<str>,
  status: watch::Sender<RunnerStatus>,
  connection: Option<Connection>,
  /// Asks the runner to stop while no tab shows its project.
  lingering: Option<Lingering>,
  /// Requests under way that give the runner work; it is not asked to stop meanwhile.
  pins: usize,
}

impl PartialEq for Runners {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl lurq::app::component::DevtoolsInspectable for Runners {}

impl Runners {
  pub fn new(runner_exe: PathBuf, runtime: Handle, activity: Activity, previous_end: SessionEnd) -> Self {
    Self {
      inner: Arc::new(Inner {
        runner_exe,
        runtime,
        previous_end,
        shared: Arc::new(Shared {
          displayed: Mutex::new(None),
          published: watch::Sender::new(RunnerStatus::none()),
          work: watch::Sender::new(0),
          counters: Mutex::new(HashMap::new()),
          activity,
        }),
        projects: Mutex::new(HashMap::new()),
        tabbed: Mutex::new(HashSet::new()),
        closing: AtomicBool::new(false),
      }),
    }
  }

  /// Makes `project` the displayed project: starts its runner unless it already has one. The
  /// runner of the project shown before keeps running in the background. `None` shows none.
  pub fn show(&self, project: Option<&OpenProject>) {
    let root = project.map(|project| project.repo_root.clone());
    let changed = {
      let mut displayed = self.inner.shared.displayed.lock();
      let changed = *displayed != root;
      displayed.clone_from(&root);
      changed
    };
    if let Some(project) = project {
      self.ensure(&project.repo_root, project.project.name().as_str());
    }
    if !changed {
      return;
    }
    let status = root
      .as_deref()
      .and_then(|root| {
        self
          .inner
          .projects
          .lock()
          .get(root)
          .map(|entry| entry.status.borrow().clone())
      })
      .unwrap_or_else(RunnerStatus::none);
    self.inner.shared.published.send_replace(status);
  }

  /// Starts the runner of project `root`, named `name`, unless it has one.
  pub fn ensure(&self, root: &Path, name: &str) {
    if self.inner.closing.load(Ordering::Acquire) {
      return;
    }
    let mut projects = self.inner.projects.lock();
    if projects.contains_key(root) {
      return;
    }
    tracing::info!(project = %root.display(), "starting the runner");
    let status = watch::Sender::new(RunnerStatus::starting(1, root.to_path_buf()));
    let session = AttachSession::Fresh {
      previous_end: self.inner.previous_end,
    };
    let name: Arc<str> = name.into();
    let connection = self.connect(root, &name, &status, session);
    self.inner.shared.publish(&status.borrow());
    projects.insert(
      root.to_path_buf(),
      ProjectRunner {
        name,
        status,
        connection: Some(connection),
        lingering: None,
        pins: 0,
      },
    );
  }

  /// The status of project `root`'s runner; `None` when it has not been started in this session.
  pub fn status_of(&self, root: &Path) -> Option<RunnerStatus> {
    self
      .inner
      .projects
      .lock()
      .get(root)
      .map(|entry| entry.status.borrow().clone())
  }

  /// The project `root` was removed from the workspace: its runner, if it has one, stops at
  /// checkpoints and is forgotten, so opening the project again starts a new one. All projects
  /// removes only a project without agents at work.
  pub fn forget(&self, root: &Path) {
    let Some(entry) = self.inner.projects.lock().remove(root) else {
      return;
    };
    let Some(connection) = entry.connection else {
      return;
    };
    let project = root.to_path_buf();
    self.inner.runtime.spawn(async move {
      let outcome = connection.stop(Some(QUIET_STOP_TIMEOUT)).await;
      tracing::info!(project = %project.display(), ?outcome, "stopped the runner of a removed project");
    });
  }

  /// Starts the displayed project's runner again after it failed; the reconnect attempts start
  /// over, and it attaches as a reconnect.
  pub fn restart(&self) {
    if self.inner.closing.load(Ordering::Acquire) {
      return;
    }
    let Some(root) = self.displayed() else {
      return;
    };
    let mut projects = self.inner.projects.lock();
    let Some(entry) = projects.get_mut(&root) else {
      return;
    };
    let restarted = entry.status.send_if_modified(|status| {
      if !status.state().can_restart() || !status.apply(RunnerEvent::RestartRequested) {
        return false;
      }
      status.restarted(status.generation() + 1);
      true
    });
    if !restarted {
      return;
    }
    tracing::info!(project = %root.display(), "restarting the runner");
    self.inner.shared.publish(&entry.status.borrow());
    // The previous task has ended; its handle only needs to go.
    drop(entry.connection.take());
    entry.connection = Some(self.connect(&root, &entry.name, &entry.status, AttachSession::Reconnect));
  }

  fn connect(
    &self,
    root: &Path,
    name: &Arc<str>,
    status: &watch::Sender<RunnerStatus>,
    session: AttachSession,
  ) -> Connection {
    let generation = status.borrow().generation();
    Connection::spawn(
      &self.inner.runtime,
      self.inner.runner_exe.clone(),
      root.to_path_buf(),
      Reporter::new(generation, name.clone(), status.clone(), self.inner.shared.clone()),
      session,
    )
  }

  fn displayed(&self) -> Option<PathBuf> {
    self.inner.shared.displayed.lock().clone()
  }

  /// Opens today's runner log of the displayed project with its default program, or the logs
  /// folder when the runner has not written a log today.
  pub fn open_log(&self) {
    if let Some(project) = self.displayed() {
      self.open_log_of(&project);
    }
  }

  /// Opens today's runner log of project `root`, or its logs folder.
  pub fn open_log_of(&self, root: &Path) {
    let project = root.to_path_buf();
    self.inner.runtime.spawn_blocking(move || {
      if let Err(error) = open_log(&project) {
        tracing::error!(project = %project.display(), %error, "cannot open the runner log");
      }
    });
  }

  /// The displayed project's runner status, whenever it changes.
  pub fn subscribe(&self) -> watch::Receiver<RunnerStatus> {
    self.inner.shared.published.subscribe()
  }

  pub fn status(&self) -> RunnerStatus {
    self.inner.shared.published.borrow().clone()
  }

  /// Changes whenever project `root`'s runner reports that a run changed state.
  pub fn states_of(&self, root: &Path) -> watch::Receiver<u64> {
    self.inner.shared.counters(root).states.subscribe()
  }

  /// Changes whenever project `root`'s runner reports that a run or its transcript changed.
  pub fn changes_of(&self, root: &Path) -> watch::Receiver<u64> {
    self.inner.shared.counters(root).changes.subscribe()
  }

  /// Changes whenever a runner reports what it works on, or its state changes.
  pub fn work_changes(&self) -> watch::Receiver<u64> {
    self.inner.shared.work.subscribe()
  }

  /// Asks the displayed project's runner to start a run of queued task `task`. Idempotent by
  /// `key`: retrying after a lost reply answers with the run the first request created.
  pub async fn start_run(&self, key: IdempotencyKey, task: TaskId) -> Result<RunId, RunRequestError> {
    match self
      .ask(self.displayed().as_deref(), Command::StartRun { key, task })
      .await?
    {
      Reply::RunStarted { run } => Ok(run),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to StartRun");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Asks the displayed project's runner to start the next run of task `task` after its latest
  /// run `after` failed or was cancelled (Run again), removing `remove_worktrees`, what `after`
  /// left, first. Idempotent by `key`, like [`Runners::start_run`].
  pub async fn run_again(
    &self,
    key: IdempotencyKey,
    task: TaskId,
    after: RunId,
    remove_worktrees: Option<Leftovers>,
  ) -> Result<RunId, RunRequestError> {
    let command = Command::RunAgain {
      key,
      task,
      after,
      remove_worktrees,
    };
    match self.ask(self.displayed().as_deref(), command).await? {
      Reply::RunStarted { run } => Ok(run),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to RunAgain");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Asks the displayed project's runner to pause, resume or cancel a run, or to raise its
  /// limits (`command`); returns once the runner has carried it out.
  pub async fn control_run(&self, command: Command) -> Result<(), RunRequestError> {
    match self.ask(self.displayed().as_deref(), command).await? {
      Reply::Accepted { .. } => Ok(()),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to a run control request");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Asks the displayed project's runner to continue interrupted run `run` from its last
  /// checkpoint; returns once it runs again.
  pub async fn continue_run(&self, run: RunId) -> Result<(), RunRequestError> {
    let command = Command::ContinueRuns { runs: vec![run] };
    match self.ask(self.displayed().as_deref(), command).await? {
      Reply::Continued { refused, .. } => match refused.into_iter().next() {
        Some(refused) => Err(RunRequestError::Rejected(refused.rejection)),
        None => Ok(()),
      },
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to ContinueRuns");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Continues interrupted runs `runs` of project `root` from their last checkpoints, starting
  /// its runner (named `name`) if needed and waiting for it to attach.
  pub async fn continue_runs(&self, root: &Path, name: &str, runs: Vec<RunId>) -> Result<Continued, RunRequestError> {
    self.ensure(root, name);
    // A project no tab shows keeps its runner for the runs it continues, then lets it stop.
    let _pin = self.pin(root);
    self.linger_untabbed();
    self.attached(root).await?;
    match self.ask(Some(root), Command::ContinueRuns { runs }).await? {
      Reply::Continued { continued, refused } => Ok(Continued { continued, refused }),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to ContinueRuns");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Waits until the runner of `root` has attached (it reported its work), or gave up.
  async fn attached(&self, root: &Path) -> Result<(), RequestError> {
    let Some(mut updates) = self
      .inner
      .projects
      .lock()
      .get(root)
      .map(|entry| entry.status.subscribe())
    else {
      return Err(RequestError::NotConnected);
    };
    let ready = async {
      loop {
        {
          let status = updates.borrow_and_update();
          match status.state() {
            RunnerState::Online if status.work().is_some() => return Ok(()),
            RunnerState::Failed { .. } | RunnerState::Stopped => return Err(RequestError::NotConnected),
            _ => {}
          }
        }
        if updates.changed().await.is_err() {
          return Err(RequestError::NotConnected);
        }
      }
    };
    tokio::time::timeout(CONNECT_TIMEOUT, ready)
      .await
      .unwrap_or(Err(RequestError::NotConnected))
  }

  async fn ask(&self, root: Option<&Path>, command: Command) -> Result<Reply, RequestError> {
    let answer = {
      let projects = self.inner.projects.lock();
      match root
        .and_then(|root| projects.get(root))
        .and_then(|entry| entry.connection.as_ref())
      {
        Some(connection) => connection.request(command),
        None => return Err(RequestError::NotConnected),
      }
    };
    // The task ended without answering, e.g. when the runner stopped meanwhile.
    answer.await.unwrap_or(Err(RequestError::Lost))
  }

  /// Work that closing would interrupt: every run with an agent at work, across the projects'
  /// runners, as they last reported it.
  pub fn active_work(&self) -> Vec<ActiveWork> {
    let projects = self.inner.projects.lock();
    let mut entries: Vec<&ProjectRunner> = projects.values().collect();
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
      .into_iter()
      .flat_map(|entry| {
        let status = entry.status.borrow();
        let runs: Vec<ActiveWork> = match (status.state(), status.work()) {
          (RunnerState::Online, Some(work)) => work
            .runs
            .iter()
            .filter(|run| !matches!(run.state, WorkState::Saved { .. }))
            .map(|run| ActiveWork::reported(&entry.name, run))
            .collect(),
          _ => Vec::new(),
        };
        runs
      })
      .collect()
  }

  /// Where `run` stands while the application closes after "Wait for agents to save", from its
  /// runner's latest report; `None` when no runner reported it.
  pub fn save_state(&self, run: RunId) -> Option<SaveState> {
    self.inner.projects.lock().values().find_map(|entry| {
      entry
        .status
        .borrow()
        .work()
        .and_then(|work| work.runs.iter().find(|work| work.run == run))
        .map(|work| SaveState::of(&work.state))
    })
  }

  /// Stops every runner for the application to close: at checkpoints, waiting at most
  /// `deadline` for each when given. No runner starts afterwards.
  pub async fn shutdown(&self, deadline: Option<Duration>) -> ShutdownOutcome {
    self.inner.closing.store(true, Ordering::Release);
    let connections: Vec<Connection> = self
      .inner
      .projects
      .lock()
      .values_mut()
      .filter_map(|entry| entry.connection.take())
      .collect();
    let stopping: Vec<_> = connections
      .into_iter()
      .map(|connection| self.inner.runtime.spawn(connection.stop(deadline)))
      .collect();
    let mut outcome = ShutdownOutcome::NoRunner;
    for stop in stopping {
      let stopped = stop.await.unwrap_or_else(|error| {
        tracing::error!(%error, "stopping a runner failed");
        ShutdownOutcome::Unanswered
      });
      outcome = match (outcome, stopped) {
        (ShutdownOutcome::Unanswered, _) | (_, ShutdownOutcome::Unanswered) => ShutdownOutcome::Unanswered,
        (ShutdownOutcome::Stopped, _) | (_, ShutdownOutcome::Stopped) => ShutdownOutcome::Stopped,
        (ShutdownOutcome::NoRunner, ShutdownOutcome::NoRunner) => ShutdownOutcome::NoRunner,
      };
    }
    outcome
  }

  /// "Quit now": terminates every runner and its agents without a checkpoint.
  pub fn terminate(&self) {
    self.inner.closing.store(true, Ordering::Release);
    let connections: Vec<Connection> = self
      .inner
      .projects
      .lock()
      .values_mut()
      .filter_map(|entry| entry.connection.take())
      .collect();
    drop(connections);
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
