//! Orders the saves of one draft that run on the blocking pool. An editor starts a save for every change of its fields
//! and does not wait for the last one before it starts the next, so two saves of one draft can be on the pool at once
//! and finish in either order; the one that finishes last would win with older text, or throw away a draft the newer
//! save just wrote. A lane gives every save a ticket when it starts and lets only the newest ticket write, alone.
//!
//! A save that is skipped leaves its text to a newer one, so a newer save must always reach the pool: whoever takes a
//! ticket submits its save to the pool in the same step, and never from a future that can be dropped first.
//! [`crate::sharing::Servers::save_lane`] keeps one lane per resource, so it outlives the panel that started the saves.

use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;

/// The saves of one draft.
#[derive(Debug, Default)]
pub struct SaveLane {
  started: AtomicU64,
  /// Held for the whole of a save, which is blocking work with no `.await` in it.
  writing: Mutex<()>,
}

/// The place of a save among the saves of its lane, taken when it starts.
#[derive(Clone, Copy, Debug)]
pub struct Ticket(u64);

impl SaveLane {
  /// A save starts: the ones that started before it are out of date.
  pub fn start(&self) -> Ticket {
    Ticket(self.started.fetch_add(1, Ordering::SeqCst) + 1)
  }

  /// Runs `save` alone, unless a save started after `ticket`'s: then what `save` writes is already out of date, and
  /// nothing is written.
  pub fn run_if_newest<T>(&self, ticket: Ticket, save: impl FnOnce() -> T) -> Option<T> {
    let _held = self.writing.lock();
    (self.started.load(Ordering::SeqCst) == ticket.0).then(save)
  }

  /// Runs `save` alone, whatever started before it, which is out of date from here on: a save of what a person pressed
  /// *Publish* with is never skipped, and no older save writes after it.
  pub fn run_newest<T>(&self, save: impl FnOnce() -> T) -> T {
    self.start();
    let _held = self.writing.lock();
    save()
  }
}
