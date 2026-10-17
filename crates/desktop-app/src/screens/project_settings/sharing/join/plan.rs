//! What the review's ticks come to: which tasks go up, under which new keys, which stay *Not shared*, and which go
//! up claimed for the person. The server numbers an import in its order, the local tasks' order, from the project's
//! next number; a task with a run that has not ended always goes up.

use std::collections::BTreeSet;

use orchester_core::{
  id::TaskId,
  project::{ProjectKey, TaskKey},
};
use orchester_workspace::sharing::project::{JoinReview, ReviewTask};

/// The tasks the person unticked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Unticked(pub BTreeSet<TaskId>);

impl lurq::app::component::DevtoolsInspectable for Unticked {}

/// One task of the review as the ticks leave it.
pub(super) struct Planned<'a> {
  pub task: &'a ReviewTask,
  pub ticked: bool,
  /// Its new key, when it goes up.
  pub becomes: Option<TaskKey>,
}

/// The review as the ticks leave it.
pub(super) struct Plan<'a> {
  pub rows: Vec<Planned<'a>>,
}

impl<'a> Plan<'a> {
  pub(super) fn of(review: &'a JoinReview, unticked: &Unticked) -> Self {
    let mut next = review.next;
    let rows = review
      .tasks
      .iter()
      .map(|task| {
        let ticked = task.locked() || !unticked.0.contains(&task.task.id());
        let becomes = if ticked {
          next += 1;
          key(&review.key, next - 1)
        } else {
          None
        };
        Planned { task, ticked, becomes }
      })
      .collect();
    Self { rows }
  }

  /// The tasks that go up, in order.
  pub(super) fn ticked(&self) -> Vec<TaskId> {
    self
      .rows
      .iter()
      .filter(|row| row.ticked)
      .map(|row| row.task.task.id())
      .collect()
  }

  /// The new keys of the tasks that go up.
  pub(super) fn keys(&self) -> Vec<&TaskKey> {
    self.rows.iter().filter_map(|row| row.becomes.as_ref()).collect()
  }

  /// How many stay *Not shared*.
  pub(super) fn kept(&self) -> usize {
    self.rows.iter().filter(|row| !row.ticked).count()
  }

  /// How many go up claimed for the person: those with a run that has not ended.
  pub(super) fn claimed(&self) -> usize {
    self.rows.iter().filter(|row| row.task.locked()).count()
  }
}

/// The key `number` takes in `project`; none for 0, which the server never gives.
fn key(project: &ProjectKey, number: u32) -> Option<TaskKey> {
  std::num::NonZeroU32::new(number).map(|number| TaskKey::new(project.clone(), number))
}
