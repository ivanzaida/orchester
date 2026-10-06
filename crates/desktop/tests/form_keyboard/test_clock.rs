//! The clock the harness hands the desktop: the computer's time until a test sets it, and from
//! then on only what the test sets. It counts how often the window read it: a live value reads it
//! each time it is rendered again, so the count is how many redraws the live values asked for.

use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_core::time::Timestamp;
use orchester_desktop::clock::TimeSource;
use parking_lot::Mutex;

#[derive(Default)]
pub struct TestClock {
  set: Mutex<Option<Timestamp>>,
  reads: AtomicUsize,
}

impl TestClock {
  /// From now on the clock tells `at`.
  pub fn set(&self, at: Timestamp) {
    *self.set.lock() = Some(at);
  }

  /// How often the window read the clock since the last call.
  pub fn take_reads(&self) -> usize {
    self.reads.swap(0, Ordering::Relaxed)
  }
}

impl TimeSource for TestClock {
  fn now(&self) -> Timestamp {
    self.reads.fetch_add(1, Ordering::Relaxed);
    self.set.lock().unwrap_or_else(Timestamp::now)
  }
}
