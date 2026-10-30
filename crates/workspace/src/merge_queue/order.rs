//! A person's say in the queue's order (decided 2026-10-01): a run that waits its turn moves up one place or to the
//! top of the waiting runs of its lane, or leaves the queue and is ready for review again. There is no dragging, and
//! only waiting runs move: the run being delivered and the ones waiting for approval keep their places, and so do the
//! runs of the other lanes.

use std::path::Path;

use orchester_core::{
  id::RunId,
  merge_queue::{QueueEntry, QueueEvent, QueueState},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{QueueRefusal, wake};

/// Where a waiting run moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
  /// One place up among the waiting runs of its lane.
  Up,
  /// Before every waiting run of its lane.
  ToTop,
}

/// Moves `run`, which waits its turn in the queue of the project at `root`, as `how` says, among the waiting runs of
/// its lane. The first waiting run of a lane does not move up.
pub fn move_run(root: &Path, run: RunId, how: Move) -> Result<(), QueueRefusal> {
  let mut db = ProjectDb::open(root)?;
  let entries = db.merge_queue()?;
  let lane = entries
    .iter()
    .find(|entry| entry.run == run && entry.state == QueueState::Queued)
    .ok_or(QueueRefusal::NotThere)?
    .base
    .clone();
  let in_lane = |entry: &QueueEntry| entry.state == QueueState::Queued && entry.base == lane;
  let mut waiting: Vec<RunId> = entries
    .iter()
    .filter(|entry| in_lane(entry))
    .map(|entry| entry.run)
    .collect();
  let at = waiting
    .iter()
    .position(|waits| *waits == run)
    .ok_or(QueueRefusal::NotThere)?;
  if at == 0 {
    return Err(QueueRefusal::First);
  }
  match how {
    Move::Up => waiting.swap(at - 1, at),
    Move::ToTop => {
      let moved = waiting.remove(at);
      waiting.insert(0, moved);
    }
  }
  let mut moved = waiting.into_iter();
  let order: Vec<RunId> = entries
    .iter()
    .filter(|entry| entry.state.in_queue())
    .map(|entry| {
      if in_lane(entry) {
        moved.next().expect("as many waiting runs as before")
      } else {
        entry.run
      }
    })
    .collect();
  db.order_merge_queue(&order)?;
  wake(root);
  Ok(())
}

/// Puts `run`, which waits its turn in the queue in `db`, before every other waiting run, and so first in its lane.
pub(super) fn to_top(db: &mut ProjectDb, run: RunId) -> Result<(), orchester_store::StoreError> {
  let entries = db.merge_queue()?;
  let others: Vec<&QueueEntry> = entries
    .iter()
    .filter(|entry| entry.state.in_queue() && entry.run != run)
    .collect();
  let first_waiting = others
    .iter()
    .position(|entry| entry.state == QueueState::Queued)
    .unwrap_or(others.len());
  let mut order: Vec<RunId> = others.iter().map(|entry| entry.run).collect();
  order.insert(first_waiting, run);
  db.order_merge_queue(&order)
}

/// Takes `run`, which waits its turn in the queue of the project at `root`, out of it at `now`: it is ready for review
/// again, a person merges it from its task page, and it does not join the queue again.
pub fn remove(root: &Path, run: RunId, now: Timestamp) -> Result<(), QueueRefusal> {
  let mut db = ProjectDb::open(root)?;
  let mut entry = db.merge_queue_entry(run)?.ok_or(QueueRefusal::NotThere)?;
  if entry.state != QueueState::Queued {
    return Err(QueueRefusal::NotThere);
  }
  entry.state = QueueState::Removed;
  entry.since = now;
  if !db.save_merge_queue_entry(&entry, &QueueState::Queued, (&[QueueEvent::Removed], now))? {
    return Err(QueueRefusal::NotThere);
  }
  wake(root);
  Ok(())
}
