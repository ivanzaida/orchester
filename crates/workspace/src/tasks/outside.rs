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
  /// The project's auto mode settings changed.
  AutoMode,
  /// Whether the project uses the code index changed.
  CodeIndex,
  /// The project's limits changed.
  Limits,
  /// A pipeline was added or published.
  Pipelines,
  /// A connection or an MCP server was added, changed, checked or removed.
  Connections,
  /// A command tool was added, saved or published.
  Tools,
}

/// How often each kind of a project's data changed outside the screens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutsideCounts {
  pub tasks: u64,
  pub runs: u64,
  pub auto_mode: u64,
  pub code_index: u64,
  pub limits: u64,
  pub pipelines: u64,
  pub connections: u64,
  pub tools: u64,
}

impl OutsideCounts {
  /// How often `change` was made.
  pub const fn of(self, change: Outside) -> u64 {
    match change {
      Outside::Tasks => self.tasks,
      Outside::Runs => self.runs,
      Outside::AutoMode => self.auto_mode,
      Outside::CodeIndex => self.code_index,
      Outside::Limits => self.limits,
      Outside::Pipelines => self.pipelines,
      Outside::Connections => self.connections,
      Outside::Tools => self.tools,
    }
  }

  fn count(&mut self, change: Outside) {
    let count = match change {
      Outside::Tasks => &mut self.tasks,
      Outside::Runs => &mut self.runs,
      Outside::AutoMode => &mut self.auto_mode,
      Outside::CodeIndex => &mut self.code_index,
      Outside::Limits => &mut self.limits,
      Outside::Pipelines => &mut self.pipelines,
      Outside::Connections => &mut self.connections,
      Outside::Tools => &mut self.tools,
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

impl OutsideChanges {
  /// `change` was made to `project` outside the screens.
  pub fn record(&self, project: ProjectId, change: Outside) {
    self
      .0
      .send_modify(|counts| counts.entry(project).or_default().count(change));
  }

  pub fn subscribe(&self) -> watch::Receiver<HashMap<ProjectId, OutsideCounts>> {
    self.0.subscribe()
  }
}
