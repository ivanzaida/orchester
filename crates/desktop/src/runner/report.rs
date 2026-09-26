//! The published runner status and the reporter a runner task uses to change it.

use std::{
  path::{Path, PathBuf},
  time::{Instant, SystemTime},
};

use tokio::sync::watch;

use super::state::{RunnerEvent, RunnerState};
use crate::{
  activity::{Activity, Target},
  messages::Message,
};

/// The runner of the open project, as the UI shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerStatus {
  /// Changes whenever a runner is started for another project, so a task of a replaced runner
  /// cannot overwrite the status of its successor.
  generation: u64,
  project: Option<PathBuf>,
  state: RunnerState,
  /// When the next reconnect attempt starts, while reconnecting.
  until: Option<Instant>,
  /// When the current problem (reconnecting, then failed) began.
  trouble_since: Option<SystemTime>,
}

impl RunnerStatus {
  /// No project is open.
  pub(super) fn none(generation: u64) -> Self {
    Self {
      generation,
      project: None,
      state: RunnerState::Stopped,
      until: None,
      trouble_since: None,
    }
  }

  pub(super) fn starting(generation: u64, project: PathBuf) -> Self {
    Self {
      generation,
      project: Some(project),
      state: RunnerState::Starting,
      until: None,
      trouble_since: None,
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

  /// The state of the runner for `project`; a runner for another project does not count, and
  /// one that has not been registered yet is about to start.
  pub fn state_for(&self, project: &Path) -> RunnerState {
    if self.project.as_deref() == Some(project) {
      self.state.clone()
    } else {
      RunnerState::Starting
    }
  }

  pub(super) fn apply(&mut self, event: RunnerEvent) -> bool {
    let next = self.state.clone().apply(event);
    if next == self.state {
      return false;
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

/// Applies a runner task's events to the status while that task's runner is still current,
/// and records a reconnect as the latest event (state F in `keJ9Z`).
#[derive(Clone)]
pub(super) struct Reporter {
  generation: u64,
  status: watch::Sender<RunnerStatus>,
  activity: Activity,
  runs: watch::Sender<u64>,
}

impl Reporter {
  pub(super) fn new(
    generation: u64,
    status: watch::Sender<RunnerStatus>,
    activity: Activity,
    runs: watch::Sender<u64>,
  ) -> Self {
    Self {
      generation,
      status,
      activity,
      runs,
    }
  }

  /// The runner changed a run or its transcript: screens showing runs read them again.
  pub(super) fn runs_changed(&self) {
    if self.status.borrow().generation == self.generation {
      self.runs.send_modify(|revision| *revision += 1);
    }
  }

  /// The state after `event`, or `None` when this runner has been replaced.
  pub(super) fn report(&self, event: RunnerEvent) -> Option<RunnerState> {
    let mut current = None;
    let mut reconnected = false;
    self.status.send_if_modified(|status| {
      if status.generation != self.generation {
        return false;
      }
      let was_reconnecting = matches!(status.state, RunnerState::Reconnecting { .. });
      let changed = status.apply(event);
      reconnected = was_reconnecting && status.state == RunnerState::Online;
      if changed {
        tracing::info!(project = ?status.project, state = ?status.state, "runner state");
      }
      current = Some(status.state.clone());
      changed
    });
    if reconnected {
      self
        .activity
        .record(Message::new("statusBar.activity.runnerReconnected"), Target::RunnerLog);
    }
    current
  }
}
