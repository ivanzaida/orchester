//! The list's tasks as one render shows them, kept between renders: the tasks as they stand with the
//! project's runner and the workflow tabs' counts, until the feed reads the tasks again or the runner comes
//! or goes; the rows, until the tab, the search, the pipeline or the sort changes too. A render for anything
//! else (a hover, the runner's status line) reuses them.

use std::sync::Arc;

use orchester_workspace::{
  modules::ModuleFilter,
  tasks::{
    browse::{PipelineFilter, TaskSort},
    data::ProjectTasks,
    list::{TaskRow, TaskTab},
  },
};
use parking_lot::Mutex;

use crate::routes::TaskListTab;

/// How many tasks each workflow tab holds, whatever the search and the pipeline filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TabCounts([usize; TaskTab::ALL.len()]);

impl TabCounts {
  /// Every tab counted as `list::rows` fills it: All holds every task, each workflow tab the tasks of its
  /// status, and Needs you the tasks something of waits for ([`TaskTab::count`]).
  fn of(tasks: &ProjectTasks) -> Self {
    Self(TaskTab::ALL.map(|tab| tab.count(tasks)))
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
  /// The *Not shared* tasks in place of the list's.
  pub not_shared: bool,
  pub search: String,
  pub pipeline: PipelineFilter,
  pub module: ModuleFilter,
  pub sort: TaskSort,
}

struct Kept {
  /// The feed's tasks they come from, whether the runner served the project, and whether they are the *Not shared*
  /// ones.
  source: ProjectTasks,
  online: bool,
  not_shared: bool,
  shown: ProjectTasks,
  counts: TabCounts,
  rows: Option<(RowsKey, Arc<Vec<TaskRow>>)>,
}

#[derive(Default)]
pub(super) struct ListView(Mutex<Option<Kept>>);

impl ListView {
  /// The feed's `source` tasks as they stand with the runner (`online` or not), or its *Not shared* ones, with the
  /// tabs' counts.
  pub(super) fn tasks(&self, source: &ProjectTasks, (online, not_shared): (bool, bool)) -> (ProjectTasks, TabCounts) {
    let mut kept = self.0.lock();
    let current = kept
      .as_ref()
      .is_some_and(|kept| kept.online == online && kept.not_shared == not_shared && same_reading(&kept.source, source));
    if !current {
      let listed = if not_shared {
        source.not_shared_view()
      } else {
        source.clone()
      };
      let shown = if online { listed } else { listed.without_runner() };
      *kept = Some(Kept {
        source: source.clone(),
        online,
        not_shared,
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
