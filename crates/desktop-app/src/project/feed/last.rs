//! What Review's screens chose, kept in the project's feed so they show it again at once when they open again.

use std::sync::Arc;

use orchester_core::{project::TaskKey, run::RunNumber};
use orchester_workspace::tasks::merge_attempts::page::AttemptPage;

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

/// The merge attempts of runs as last read, so a page for a run opened again shows them at once while it reads them
/// anew (`Review - v2` rule 26 `rwJfp`). One page per run, the last read; kept while the window lives.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeptAttempts(Arc<Vec<(TaskKey, RunNumber, Arc<AttemptPage>)>>);

impl lurq::app::component::DevtoolsInspectable for KeptAttempts {}

impl KeptAttempts {
  /// The page last read for run `number` of `task`.
  pub fn of(&self, task: &TaskKey, number: RunNumber) -> Option<Arc<AttemptPage>> {
    self
      .0
      .iter()
      .find(|(key, run, _)| key == task && *run == number)
      .map(|(_, _, page)| page.clone())
  }

  /// `page` is what was last read for run `number` of `task`.
  pub fn keep(&mut self, task: &TaskKey, number: RunNumber, page: &Arc<AttemptPage>) {
    let pages = Arc::make_mut(&mut self.0);
    pages.retain(|(key, run, _)| key != task || *run != number);
    pages.push((task.clone(), number, page.clone()));
  }
}
