//! The list's tasks as one render shows them, kept between renders: the tasks as they stand with the project's
//! runner, what waits for the person in them and how many tasks each view holds, until the feed reads the tasks again
//! or the runner comes or goes; the rows as searched, filtered and ordered, until the search, a filter or the sort
//! changes too. A render for anything else (a hover, the runner's status line) reuses them.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use orchester_core::id::TaskId;
use orchester_workspace::tasks::{
  attention::NeedItem,
  browse::{Facets, TaskSort},
  data::ProjectTasks,
  list::TaskRow,
  views::ListView,
};
use parking_lot::Mutex;

use super::{phrase, views::list_view};
use crate::routes::TaskListTab;

/// What one reading of the tasks gives the list.
pub(super) struct Reading {
  /// The feed's tasks as they stand with the runner, or its *Not shared* ones.
  pub tasks: ProjectTasks,
  /// The tasks something of waits for the person: what every *Needs you* names.
  pub waiting: HashSet<TaskId>,
  /// The oldest thing each of those tasks waits for, which its row says and its action opens.
  pub needs: HashMap<TaskId, NeedItem>,
}

impl Reading {
  fn of(tasks: ProjectTasks) -> Self {
    Self {
      waiting: tasks.waiting_tasks(),
      needs: phrase::needs(&tasks),
      tasks,
    }
  }
}

/// How many tasks each view holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ViewCounts([usize; ListView::ALL.len()]);

impl ViewCounts {
  fn of(rows: &[TaskRow], waiting: &HashSet<TaskId>) -> Self {
    Self(ListView::ALL.map(|view| view.count(rows, waiting)))
  }

  pub(super) fn get(self, tab: TaskListTab) -> usize {
    let view = list_view(tab);
    let index = ListView::ALL
      .iter()
      .position(|counted| *counted == view)
      .expect("every view is counted");
    self.0[index]
  }
}

/// What the rows are read for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RowsKey {
  /// The *Not shared* tasks in place of the list's.
  pub not_shared: bool,
  pub search: String,
  pub facets: Facets,
  pub sort: TaskSort,
}

/// The rows a key asks for, with how many of them each view holds.
#[derive(Clone)]
pub(super) struct Rows {
  pub rows: Arc<Vec<TaskRow>>,
  pub counts: ViewCounts,
}

struct Kept {
  /// The feed's tasks they come from, whether the runner served the project, and whether they are the *Not shared*
  /// ones.
  source: ProjectTasks,
  online: bool,
  not_shared: bool,
  reading: Arc<Reading>,
  /// How many tasks each view holds whatever the search and the filters.
  totals: ViewCounts,
  rows: Option<(RowsKey, Rows)>,
}

#[derive(Default)]
pub(super) struct ListCache(Mutex<Option<Kept>>);

impl ListCache {
  /// The feed's `source` tasks as they stand with the runner (`online` or not), or its *Not shared* ones, with how
  /// many tasks each view holds; `every` reads all the rows of tasks to count them.
  pub(super) fn reading(
    &self,
    source: &ProjectTasks,
    (online, not_shared): (bool, bool),
    every: impl FnOnce(&ProjectTasks) -> Vec<TaskRow>,
  ) -> (Arc<Reading>, ViewCounts) {
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
      let reading = Reading::of(shown);
      let totals = ViewCounts::of(&every(&reading.tasks), &reading.waiting);
      *kept = Some(Kept {
        source: source.clone(),
        online,
        not_shared,
        reading: Arc::new(reading),
        totals,
        rows: None,
      });
    }
    let kept = kept.as_ref().expect("kept just above");
    (kept.reading.clone(), kept.totals)
  }

  /// The rows `key` asks for, read by `read` from the tasks [`ListCache::reading`] last gave when they were not read
  /// for the same key.
  pub(super) fn rows(&self, key: RowsKey, read: impl FnOnce(&ProjectTasks) -> Vec<TaskRow>) -> Rows {
    let mut kept = self.0.lock();
    let kept = kept.as_mut().expect("the tasks are kept before their rows are read");
    if let Some((read_for, rows)) = &kept.rows
      && *read_for == key
    {
      return rows.clone();
    }
    let rows = read(&kept.reading.tasks);
    let counts = ViewCounts::of(&rows, &kept.reading.waiting);
    let rows = Rows {
      rows: Arc::new(rows),
      counts,
    };
    kept.rows = Some((key, rows.clone()));
    rows
  }
}

/// Whether two of the feed's readings are the same one, with the same answers sent from this window: a reading again
/// replaces every part at once, and an answer changes what waits.
fn same_reading(left: &ProjectTasks, right: &ProjectTasks) -> bool {
  Arc::ptr_eq(&left.tasks, &right.tasks)
    && Arc::ptr_eq(&left.phases, &right.phases)
    && Arc::ptr_eq(&left.answered, &right.answered)
}
