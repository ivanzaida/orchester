//! The pager of a task's review page (`Review - v2` 02 `gSMiI`, rule 2 of `b6M3lS`): where the task stands among
//! *Waiting for you*, in the list's order, and which tasks its arrows lead to.

use orchester_core::project::TaskKey;

use super::ReviewList;

/// Where the page's task stands in *Waiting for you*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  /// The task waits for the person, at this position from 1 among the `total` that wait.
  At(usize),
  /// The task was in the group and left it (approved, sent back): the page stays on it, and the pager counts the rest.
  Left,
}

/// What the pager shows and where its arrows lead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pager {
  place: Place,
  /// How many tasks wait: the group's length.
  total: usize,
  /// Where the task stands (0-based), or stood when it left the group; what [`Pager::of`] takes as `was` next time.
  slot: usize,
  previous: Option<TaskKey>,
  next: Option<TaskKey>,
}

impl Pager {
  /// The pager of `key`'s review, over the list as read. `was` is the [`slot`](Self::slot) the page saw the task in
  /// while it waited: without it, a task that does not wait has no pager (it was opened from *With the agent* or
  /// *Approved today*).
  pub fn of(list: &ReviewList, key: &TaskKey, was: Option<usize>) -> Option<Self> {
    let waiting = &list.waiting;
    let keyed = |index: usize| waiting.get(index).map(|row| row.key.clone());
    match waiting.iter().position(|row| row.key == *key) {
      Some(index) => Some(Self {
        place: Place::At(index + 1),
        total: waiting.len(),
        slot: index,
        previous: index.checked_sub(1).and_then(keyed),
        next: keyed(index + 1),
      }),
      None => {
        let slot = was?.min(waiting.len());
        Some(Self {
          place: Place::Left,
          total: waiting.len(),
          slot,
          previous: slot.checked_sub(1).and_then(keyed),
          next: keyed(slot),
        })
      }
    }
  }

  pub const fn place(&self) -> Place {
    self.place
  }

  pub const fn total(&self) -> usize {
    self.total
  }

  /// Where the task stands in the group, counted from 0; where it stood once it left.
  pub const fn slot(&self) -> usize {
    self.slot
  }

  /// The task before this one in the group; `None` at its start.
  pub const fn previous(&self) -> Option<&TaskKey> {
    self.previous.as_ref()
  }

  /// The task after this one in the group; `None` at its end.
  pub const fn next(&self) -> Option<&TaskKey> {
    self.next.as_ref()
  }
}
