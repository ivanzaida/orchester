//! The list's tasks as one render shows them, kept between renders: the tasks as they stand with the
//! project's runner and the workflow tabs' counts, until the feed reads the tasks again or the runner comes
//! or goes; the rows, until the tab, the search, the pipeline or the sort changes too. A render for anything
//! else (a hover, the runner's status line) reuses them.

use std::sync::Arc;

use orchester_core::run::TaskPhase;
use orchester_workspace::tasks::{
  browse::{PipelineFilter, TaskSort},
  data::ProjectTasks,
  list::{Latest, TaskRow, TaskTab},
};
use parking_lot::Mutex;

use crate::routes::TaskListTab;

/// How many tasks each workflow tab holds, whatever the search and the pipeline filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TabCounts([usize; TaskTab::ALL.len()]);

impl TabCounts {
  /// Every tab counted in one pass over the tasks, as `list::rows` would fill it: All holds every task,
  /// Needs you the ones waiting for a person, each workflow tab the tasks of its status.
  fn of(tasks: &ProjectTasks) -> Self {
    let mut counts = [0; TaskTab::ALL.len()];
    for task in tasks.tasks.iter() {
      let phase = tasks
        .phases
        .get(&task.id())
        .map_or_else(|| Some(TaskPhase::of(task, None, None)), Latest::phase);
      let needs_you = phase.is_some_and(TaskPhase::needs_you);
      for (count, tab) in counts.iter_mut().zip(TaskTab::ALL) {
        if tab.includes(task) && (tab != TaskTab::NeedsYou || needs_you) {
          *count += 1;
        }
      }
    }
    Self(counts)
  }

  pub(super) fn get(self, tab: TaskListTab) -> usize {
    let tab = super::header::workspace_tab(tab);
    let index = TaskTab::ALL
      .iter()
      .position(|counted| *counted == tab)
      .expect("every tab is counted");
    self.0[index]
  }
}

/// What the rows are read for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RowsKey {
  pub tab: TaskListTab,
  pub search: String,
  pub pipeline: PipelineFilter,
  pub sort: TaskSort,
}

struct Kept {
  /// The feed's tasks they come from, and whether the runner served the project.
  source: ProjectTasks,
  online: bool,
  shown: ProjectTasks,
  counts: TabCounts,
  rows: Option<(RowsKey, Arc<Vec<TaskRow>>)>,
}

#[derive(Default)]
pub(super) struct ListView(Mutex<Option<Kept>>);

impl ListView {
  /// The feed's `source` tasks as they stand with the runner (`online` or not), with the tabs' counts.
  pub(super) fn tasks(&self, source: &ProjectTasks, online: bool) -> (ProjectTasks, TabCounts) {
    let mut kept = self.0.lock();
    let current = kept
      .as_ref()
      .is_some_and(|kept| kept.online == online && same_reading(&kept.source, source));
    if !current {
      let shown = if online {
        source.clone()
      } else {
        source.without_runner()
      };
      *kept = Some(Kept {
        source: source.clone(),
        online,
        counts: TabCounts::of(&shown),
        shown,
        rows: None,
      });
    }
    let kept = kept.as_ref().expect("kept just above");
    (kept.shown.clone(), kept.counts)
  }

  /// The rows `key` asks for, read by `read` from the tasks [`ListView::tasks`] last gave when they were
  /// not read for the same key.
  pub(super) fn rows(&self, key: RowsKey, read: impl FnOnce(&ProjectTasks) -> Vec<TaskRow>) -> Arc<Vec<TaskRow>> {
    let mut kept = self.0.lock();
    let kept = kept.as_mut().expect("the tasks are kept before their rows are read");
    if let Some((read_for, rows)) = &kept.rows
      && *read_for == key
    {
      return rows.clone();
    }
    let rows = Arc::new(read(&kept.shown));
    kept.rows = Some((key, rows.clone()));
    rows
  }
}

/// Whether two of the feed's readings are the same one: a reading again replaces every part at once.
fn same_reading(left: &ProjectTasks, right: &ProjectTasks) -> bool {
  Arc::ptr_eq(&left.tasks, &right.tasks) && Arc::ptr_eq(&left.phases, &right.phases)
}
