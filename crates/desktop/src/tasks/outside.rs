//! Changes made outside the screens, by automation through Orchester's MCP server, counted per project
//! and per kind of data: only the changed project's feed reads its data again, and a screen reads again
//! only what the change touched, in place.

use std::{collections::HashMap, sync::Arc};

use orchester_core::id::ProjectId;
use tokio::sync::watch;

/// What a change made outside the screens touched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outside {
  /// A task was saved, queued or returned to Draft, or a run's work changed what the task list shows.
  Tasks,
  /// A run's own settings changed, such as its auto mode.
  Runs,
  /// A project setting changed: auto mode, code index.
  Settings,
  /// A pipeline was added or published.
  Pipelines,
}

/// How often each kind of a project's data changed outside the screens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutsideCounts {
  pub tasks: u64,
  pub runs: u64,
  pub settings: u64,
  pub pipelines: u64,
}

impl lurq::app::component::DevtoolsInspectable for OutsideCounts {}

impl OutsideCounts {
  fn count(&mut self, change: Outside) {
    let count = match change {
      Outside::Tasks => &mut self.tasks,
      Outside::Runs => &mut self.runs,
      Outside::Settings => &mut self.settings,
      Outside::Pipelines => &mut self.pipelines,
    };
    *count += 1;
  }
}

/// Counts the changes made outside the screens, by project.
#[derive(Clone)]
pub struct OutsideChanges(Arc<watch::Sender<HashMap<ProjectId, OutsideCounts>>>);

impl Default for OutsideChanges {
  fn default() -> Self {
    Self(Arc::new(watch::Sender::new(HashMap::new())))
  }
}

impl PartialEq for OutsideChanges {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for OutsideChanges {}

impl OutsideChanges {
  /// `change` was made to `project` outside the screens.
  pub fn record(&self, project: ProjectId, change: Outside) {
    self
      .0
      .send_modify(|counts| counts.entry(project).or_default().count(change));
  }

  /// The counts of `project` so far.
  pub fn of(&self, project: ProjectId) -> OutsideCounts {
    self.0.borrow().get(&project).copied().unwrap_or_default()
  }

  pub fn subscribe(&self) -> watch::Receiver<HashMap<ProjectId, OutsideCounts>> {
    self.0.subscribe()
  }
}
