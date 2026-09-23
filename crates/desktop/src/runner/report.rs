//! The published runner status and the reporter a runner task uses to change it.

use std::path::{Path, PathBuf};

use tokio::sync::watch;

use super::state::{RunnerEvent, RunnerState};

/// The runner of the open project, as the UI shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerStatus {
  /// Changes whenever a runner is started for another project, so a task of a replaced runner
  /// cannot overwrite the status of its successor.
  generation: u64,
  project: Option<PathBuf>,
  state: RunnerState,
}

impl RunnerStatus {
  /// No project is open.
  pub(super) fn none(generation: u64) -> Self {
    Self {
      generation,
      project: None,
      state: RunnerState::Stopped,
    }
  }

  pub(super) fn starting(generation: u64, project: PathBuf) -> Self {
    Self {
      generation,
      project: Some(project),
      state: RunnerState::Starting,
    }
  }

  pub(super) fn generation(&self) -> u64 {
    self.generation
  }

  pub(super) fn project(&self) -> Option<&Path> {
    self.project.as_deref()
  }

  pub fn state(&self) -> &RunnerState {
    &self.state
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
    let changed = next != self.state;
    self.state = next;
    changed
  }
}

impl lurq::app::component::DevtoolsInspectable for RunnerStatus {}

/// Applies a runner task's events to the status while that task's runner is still current.
#[derive(Clone)]
pub(super) struct Reporter {
  generation: u64,
  status: watch::Sender<RunnerStatus>,
}

impl Reporter {
  pub(super) fn new(generation: u64, status: watch::Sender<RunnerStatus>) -> Self {
    Self { generation, status }
  }

  /// The state after `event`, or `None` when this runner has been replaced.
  pub(super) fn report(&self, event: RunnerEvent) -> Option<RunnerState> {
    let mut current = None;
    self.status.send_if_modified(|status| {
      if status.generation != self.generation {
        return false;
      }
      let changed = status.apply(event);
      if changed {
        tracing::info!(project = ?status.project, state = ?status.state, "runner state");
      }
      current = Some(status.state.clone());
      changed
    });
    current
  }
}
