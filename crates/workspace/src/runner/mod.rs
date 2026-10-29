//! The runners of the open projects (`docs/architecture/storage.md`, "Processes"): one
//! `orchester-runner` per project a window tab shows or whose runner has work, started when the
//! project opens in a tab, kept running in the background while another tab is active,
//! reconnected when it drops, stopped once no tab shows the project and it has no work (see
//! [`lifetime`]) and stopped when the application closes. The displayed project's runner status
//! is published for the screens.
//!
//! Windows only for now: the IPC client (`orchester_ipc::RunnerClient`, a named pipe and a job
//! object) exists only on Windows.

mod answers;
mod ask;
mod chat;
mod check;
mod connection;
mod estimate;
mod idle;
mod lifetime;
mod report;
mod requests;
pub mod state;
mod work;

use std::{
  collections::{HashMap, HashSet},
  io,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  time::{Duration, SystemTime},
};

use orchester_core::id::RunId;
use orchester_ipc::{AttachSession, Command, ErrorReports, RefusedRun, Rejection, Reply, SessionEnd, WorkState};
use parking_lot::Mutex;
use tokio::{runtime::Handle, sync::watch, task::JoinSet};

pub use self::{
  answers::AnswerMark,
  idle::ForgetRefusal,
  report::RunnerStatus,
  requests::RequestError,
  work::{ActiveWork, ActiveWorkFields, SaveState},
};
use self::{
  connection::Connection,
  lifetime::Lingering,
  report::{Reporter, Shared},
  state::{RunnerEvent, RunnerState},
};
use crate::{activity::Activity, workspace::OpenProject};

/// How long a close without active work waits for the runner to stop and exit before
/// terminating it.
pub const QUIET_STOP_TIMEOUT: Duration = Duration::from_secs(6);

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
  /// Whether the runners send error reports, and in which environment and release; each runner reads it when it
  /// starts.
  reports: Arc<Mutex<Option<ErrorReports>>>,
  /// Held while the setting is stored and handed to the running runners ([`Runners::set_error_reports`]).
  report_updates: Mutex<()>,
  runtime: Handle,
  /// How the previous session ended: each project's first runner attaches with it.
  previous_end: SessionEnd,
  shared: Arc<Shared>,
  projects: Mutex<HashMap<PathBuf, ProjectRunner>>,
  /// The projects the window's tabs show; the other runners linger until they have no work.
  tabbed: Mutex<HashSet<PathBuf>>,
  /// Set once the application closes; no runner starts after that.
  closing: AtomicBool,
  /// Held while a project's limits are stored and changed agent slots are handed to its runner, so
  /// the runner gets them in the order they were stored ([`Runners::store_agent_slots`]).
  slot_updates: Mutex<()>,
  /// Hands out the generations of the projects' runners.
  generations: AtomicU64,
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
  /// Which runner of the project this is: each start gets a new one.
  generation: u64,
  /// The question whether it is idle, while one is in flight (`idle`).
  asking: Option<idle::IdleAnswer>,
  /// The project is being removed from the list: once stopped, the runner is not started again.
  forgetting: bool,
}

impl PartialEq for Runners {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl Runners {
  pub fn plan_limits(&self) -> watch::Receiver<orchester_core::connection::plan_limits::PlanReadings> {
    self.inner.shared.limits.subscribe()
  }

  /// The runners of the program `runner_exe`. They send no error reports until
  /// [`Self::set_error_reports`] says so.
  pub fn new(runner_exe: PathBuf, runtime: Handle, activity: Activity, previous_end: SessionEnd) -> Self {
    Self {
      inner: Arc::new(Inner {
        runner_exe,
        reports: Arc::new(Mutex::new(None)),
        report_updates: Mutex::new(()),
        runtime,
        previous_end,
        shared: Arc::new(Shared {
          displayed: Mutex::new(None),
          published: watch::Sender::new(RunnerStatus::none()),
          work: watch::Sender::new(0),
          runs: watch::Sender::new(0),
          limits: watch::Sender::new(Default::default()),
          counters: Mutex::new(HashMap::new()),
          last_events: Mutex::new(HashMap::new()),
          activity,
        }),
        projects: Mutex::new(HashMap::new()),
        tabbed: Mutex::new(HashSet::new()),
        closing: AtomicBool::new(false),
        slot_updates: Mutex::new(()),
        generations: AtomicU64::new(0),
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
        generation: self.inner.generations.fetch_add(1, Ordering::Relaxed),
        asking: None,
        forgetting: false,
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
      self.inner.reports.clone(),
      root.to_path_buf(),
      Reporter::new(generation, name.clone(), status.clone(), self.inner.shared.clone()),
      session,
    )
  }

  /// The runners send their panics and ERROR lines as error reports with `reports`, or none with `None`
  /// (`orchester_ipc::error_reports`): every runner running now is told at once
  /// (`Command::SetErrorReports`), without waiting for its answer, and every runner started from now on starts so.
  /// A runner that is starting is told once it is connected; one that cannot be reached misses nothing, since it
  /// starts with the setting the next time.
  pub fn set_error_reports(&self, reports: Option<&ErrorReports>) {
    let reports = reports.cloned();
    // Held while storing and telling the runners, so they end with the setting stored last.
    let _in_order = self.inner.report_updates.lock();
    *self.inner.reports.lock() = reports.clone();
    let answers: Vec<_> = self
      .inner
      .projects
      .lock()
      .iter()
      .filter_map(|(root, entry)| {
        let connection = entry.connection.as_ref()?;
        let command = Command::SetErrorReports {
          reports: reports.clone(),
        };
        Some((root.clone(), connection.request(command)))
      })
      .collect();
    for (root, answer) in answers {
      self.inner.runtime.spawn(async move {
        // The task ended without answering, e.g. when the runner stopped meanwhile.
        match answer.await.unwrap_or(Err(RequestError::Lost)) {
          Ok(Reply::ErrorReportsSet) => {}
          Ok(other) => tracing::error!(?other, "unexpected reply to SetErrorReports"),
          Err(error) => {
            tracing::info!(project = %root.display(), %error, "no runner took the error reports setting now");
          }
        }
      });
    }
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

  /// Tells the readers of [`Self::changes_of`] that the desktop queued notices that project `root`'s base branch
  /// moved (`crate::base_moved`): the runs' timelines read them again. A run's state did not change.
  pub fn base_notices_queued(&self, root: &Path) {
    self
      .inner
      .shared
      .counters(root)
      .changes
      .send_modify(|revision| *revision += 1);
  }

  /// Counts a change as project `root`'s runner reports it: a run's state (`true`), which moves its transcript
  /// on too, or its transcript alone. For tests of what follows the counts, which run no runner.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn count_change_for_test(&self, root: &Path, state: bool) {
    let counters = self.inner.shared.counters(root);
    if state {
      self.inner.shared.runs.send_modify(|revision| *revision += 1);
      counters.states.send_modify(|revision| *revision += 1);
    }
    counters.changes.send_modify(|revision| *revision += 1);
  }

  /// Hands `reading`, from `source`, on to what shows the session's plan limits, as a project's runner hands on what
  /// a turn reported: a read outside a run ([`crate::plan_limits::reads`]) is a reading like a turn's.
  pub fn observe_plan_limits(
    &self,
    reading: orchester_core::connection::plan_limits::LimitReading,
    source: orchester_core::connection::plan_limits::ReadingSource,
  ) {
    self
      .inner
      .shared
      .limits
      .send_if_modified(|readings| readings.observe_from(reading, source));
  }

  /// Changes whenever any runner reports that a run changed.
  pub fn run_changes(&self) -> watch::Receiver<u64> {
    self.inner.shared.runs.subscribe()
  }

  /// Changes whenever a runner reports what it works on, or its state changes.
  pub fn work_changes(&self) -> watch::Receiver<u64> {
    self.inner.shared.work.subscribe()
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

  /// When a runner last reported an event of `run` (its agent's activity or a change of the run) in
  /// this session; `None` before the first one.
  pub fn last_event(&self, run: RunId) -> Option<SystemTime> {
    self.inner.shared.last_events.lock().get(&run).map(|(_, at)| *at)
  }

  /// Every run the runners last reported working on, with where it stands while they stop at
  /// checkpoints (saved ones included), whatever state each runner is in now, by project name.
  pub fn board(&self) -> Vec<(ActiveWork, SaveState)> {
    let projects = self.inner.projects.lock();
    let mut entries: Vec<&ProjectRunner> = projects.values().collect();
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
      .into_iter()
      .flat_map(|entry| {
        let status = entry.status.borrow();
        let runs: Vec<(ActiveWork, SaveState)> = status.work().map_or_else(Vec::new, |work| {
          work
            .runs
            .iter()
            .map(|run| (ActiveWork::reported(&entry.name, run), SaveState::of(&run.state)))
            .collect()
        });
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
    // Dropping the set, when this future is dropped (Quit now while agents save), ends every stop
    // under way and with it the runners and their agents.
    let mut stopping = JoinSet::new();
    for connection in connections {
      stopping.spawn_on(connection.stop(deadline), &self.inner.runtime);
    }
    let mut outcome = ShutdownOutcome::NoRunner;
    while let Some(stopped) = stopping.join_next().await {
      let stopped = stopped.unwrap_or_else(|error| {
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
  orchester_exec::external::open_path(&target)
}
