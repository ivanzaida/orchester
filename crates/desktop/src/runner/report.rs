//! The status of each project's runner and the reporter a runner task uses to change it. The
//! status of the displayed project is also published for the screens.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::{Instant, SystemTime},
};

use orchester_ipc::WorkSummary;
use parking_lot::Mutex;
use tokio::sync::watch;

use super::state::{RunnerEvent, RunnerState};
use crate::{
  activity::{Activity, Target},
  messages::Message,
};

/// A project's runner, as the UI shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerStatus {
  /// Changes whenever the project's runner is started again, so a task of a replaced runner
  /// cannot overwrite the status of its successor.
  generation: u64,
  project: Option<PathBuf>,
  state: RunnerState,
  /// When the next reconnect attempt starts, while reconnecting.
  until: Option<Instant>,
  /// When the current problem (reconnecting, then failed) began.
  trouble_since: Option<SystemTime>,
  /// What the runner last reported working on; `None` until it attached and after it dropped.
  work: Option<Arc<WorkSummary>>,
  /// Runs whose agents were at work when the runner dropped: interrupted until a runner is back.
  interrupted: usize,
}

impl RunnerStatus {
  /// No project is shown.
  pub(super) fn none() -> Self {
    Self {
      generation: 0,
      project: None,
      state: RunnerState::Stopped,
      until: None,
      trouble_since: None,
      work: None,
      interrupted: 0,
    }
  }

  pub(super) fn starting(generation: u64, project: PathBuf) -> Self {
    Self {
      generation,
      project: Some(project),
      state: RunnerState::Starting,
      until: None,
      trouble_since: None,
      work: None,
      interrupted: 0,
    }
  }

  pub(super) fn generation(&self) -> u64 {
    self.generation
  }

  pub fn state(&self) -> &RunnerState {
    &self.state
  }

  /// The project whose runner this is.
  pub fn project_root(&self) -> Option<&Path> {
    self.project.as_deref()
  }

  /// When the next reconnect attempt starts, while reconnecting.
  pub fn until(&self) -> Option<Instant> {
    self.until
  }

  /// When the runner's current problem began: the drop, or the failed start.
  pub fn trouble_since(&self) -> Option<SystemTime> {
    self.trouble_since
  }

  /// What the connected runner works on; `None` while that is unknown.
  pub fn work(&self) -> Option<&WorkSummary> {
    self.work.as_deref()
  }

  /// Runs interrupted by the runner's drop, while no runner is back (states E and G).
  pub fn interrupted(&self) -> usize {
    self.interrupted
  }

  /// The state of the runner for `project`; a runner for another project does not count, and
  /// one that has not been registered yet is about to start.
  pub fn state_for(&self, project: &Path) -> RunnerState {
    if self.project.as_deref() == Some(project) {
      self.state.clone()
    } else {
      RunnerState::Starting
    }
  }

  pub(super) fn restarted(&mut self, generation: u64) {
    self.generation = generation;
  }

  pub(super) fn apply(&mut self, event: RunnerEvent) -> bool {
    let next = self.state.clone().apply(event);
    if next == self.state {
      return false;
    }
    if self.state == RunnerState::Online && next != RunnerState::Stopping {
      // The runner went away while agents worked: their runs are interrupted until it is back.
      self.interrupted = self.work.as_deref().map_or(0, WorkSummary::agents_working);
    }
    if !matches!(next, RunnerState::Online | RunnerState::Stopping | RunnerState::Stopped) {
      self.work = None;
    }
    self.until = next.reconnect_delay().map(|delay| Instant::now() + delay);
    self.trouble_since = match (next.is_problem(), self.trouble_since) {
      (false, _) => None,
      (true, Some(since)) => Some(since),
      (true, None) => Some(SystemTime::now()),
    };
    self.state = next;
    true
  }
}

impl lurq::app::component::DevtoolsInspectable for RunnerStatus {}

/// What every project's reporter shares: the displayed project, whose status is published, and
/// the revisions screens follow.
pub(super) struct Shared {
  pub(super) displayed: Mutex<Option<PathBuf>>,
  pub(super) published: watch::Sender<RunnerStatus>,
  /// Counts run change notifications of all runners.
  pub(super) runs: watch::Sender<u64>,
  /// Counts work summaries of all runners (the close confirmation follows them).
  pub(super) work: watch::Sender<u64>,
  /// Where runner events (a reconnect) are recorded for the status bar.
  pub(super) activity: Activity,
}

impl Shared {
  /// Publishes `status` if its project is the displayed one.
  pub(super) fn publish(&self, status: &RunnerStatus) {
    let displayed = self.displayed.lock();
    if displayed.is_some() && displayed.as_deref() == status.project_root() {
      self.published.send_replace(status.clone());
    }
  }
}

/// Applies a runner task's events to its project's status while that task's runner is current.
#[derive(Clone)]
pub(super) struct Reporter {
  generation: u64,
  status: watch::Sender<RunnerStatus>,
  shared: Arc<Shared>,
}

impl Reporter {
  pub(super) fn new(generation: u64, status: watch::Sender<RunnerStatus>, shared: Arc<Shared>) -> Self {
    Self {
      generation,
      status,
      shared,
    }
  }

  /// The runner changed a run or its transcript: screens showing runs read them again.
  pub(super) fn runs_changed(&self) {
    self.shared.runs.send_modify(|revision| *revision += 1);
  }

  /// The state after `event`, or `None` when this runner has been replaced.
  pub(super) fn report(&self, event: RunnerEvent) -> Option<RunnerState> {
    let current = self.change(|status| {
      let changed = status.apply(event);
      if changed {
        tracing::info!(project = ?status.project, state = ?status.state, "runner state");
      }
      changed
    })?;
    self.shared.work.send_modify(|revision| *revision += 1);
    Some(current)
  }

  /// The runner attached. After a reconnect it continued `continued` interrupted runs; the
  /// reconnect becomes the latest event (state F in `keJ9Z`).
  pub(super) fn attached(&self, reconnect: bool, continued: usize) {
    self.change(|status| {
      let had = status.interrupted;
      status.interrupted = 0;
      had != 0
    });
    if !reconnect {
      return;
    }
    let title = match continued {
      0 => Message::new("statusBar.activity.runnerReconnected"),
      1 => Message::new("statusBar.activity.runnerReconnectedContinued.one"),
      count => Message::new("statusBar.activity.runnerReconnectedContinued.other").with("count", count.to_string()),
    };
    self.shared.activity.record(title, Target::RunnerLog);
  }

  /// The runner's latest work summary.
  pub(super) fn work(&self, summary: WorkSummary) {
    let summary = Arc::new(summary);
    if self
      .change(|status| {
        status.work = Some(summary);
        true
      })
      .is_some()
    {
      self.shared.work.send_modify(|revision| *revision += 1);
    }
  }

  /// Applies `change` to the status while this runner is current; the state after it.
  fn change(&self, change: impl FnOnce(&mut RunnerStatus) -> bool) -> Option<RunnerState> {
    let mut current = None;
    let modified = self.status.send_if_modified(|status| {
      if status.generation != self.generation {
        return false;
      }
      let changed = change(status);
      current = Some(status.state.clone());
      changed
    });
    if modified {
      self.shared.publish(&self.status.borrow());
    }
    current
  }
}
