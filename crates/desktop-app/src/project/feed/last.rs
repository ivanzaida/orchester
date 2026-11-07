//! What Review's screens read last and chose, kept in the project's feed so they show it again at once when they open
//! again.

use std::sync::Arc;

use orchester_core::project::TaskKey;

/// The merges Review › Merges read last, with how many pages of merged runs they hold.
#[derive(Clone, Debug, PartialEq)]
pub struct LastMerges {
  pub merges: orchester_workspace::tasks::merge_attempts::merges::Merges,
  pub pages: usize,
}

impl lurq::app::component::DevtoolsInspectable for LastMerges {}

/// The tasks whose review shows *All changes* where the branch moved after the head reviewed; the others show *Since
/// your review*. Kept while the window lives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShowingAll(Arc<Vec<TaskKey>>);

impl lurq::app::component::DevtoolsInspectable for ShowingAll {}

impl ShowingAll {
  pub fn contains(&self, task: &TaskKey) -> bool {
    self.0.contains(task)
  }

  /// `task`'s review shows all its changes, or only those since the review.
  pub fn choose(&mut self, task: &TaskKey, all: bool) {
    if self.contains(task) == all {
      return;
    }
    let tasks = Arc::make_mut(&mut self.0);
    if all {
      tasks.push(task.clone());
    } else {
      tasks.retain(|shown| shown != task);
    }
  }
}
