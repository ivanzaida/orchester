use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  time::{Duration, Instant},
};

use parking_lot::Mutex;

/// Where the watchdog reads the time: the system's monotonic clock in the app, a clock a test moves by hand.
pub trait Clock: Send + Sync {
  /// The time since a fixed start of this clock.
  fn now(&self) -> Duration;
}

/// The monotonic clock, from when it was made.
pub struct SystemClock(Instant);

impl SystemClock {
  pub fn new() -> Self {
    Self(Instant::now())
  }
}

impl Default for SystemClock {
  fn default() -> Self {
    Self::new()
  }
}

impl Clock for SystemClock {
  fn now(&self) -> Duration {
    self.0.elapsed()
  }
}

/// The window's thread's sign of life: the time of its last beat, the screen it showed then and whether the window is
/// minimized. The window's thread writes it and the watchdog reads it, without a lock but for the screen's name.
#[derive(Clone)]
pub struct Heartbeat {
  clock: Arc<dyn Clock>,
  /// The clock's milliseconds at the last beat, plus one; 0 before the first.
  last: Arc<AtomicU64>,
  minimized: Arc<AtomicBool>,
  /// The screen's kind, as the app's MCP tools name it (`Screen::name`), never which project or item it shows.
  screen: Arc<Mutex<&'static str>>,
}

impl Heartbeat {
  pub fn new(clock: Arc<dyn Clock>) -> Self {
    Self {
      clock,
      last: Arc::default(),
      minimized: Arc::default(),
      screen: Arc::new(Mutex::new("")),
    }
  }

  /// The window's thread is alive now, showing `screen`.
  pub fn beat(&self, screen: &'static str) {
    *self.screen.lock() = screen;
    let millis = u64::try_from(self.clock.now().as_millis())
      .expect("the milliseconds since the clock started fit in 64 bits for half a billion years");
    self.last.store(millis + 1, Ordering::Release);
  }

  /// Whether the window is minimized, as the window's thread last saw it.
  pub fn set_minimized(&self, minimized: bool) {
    self.minimized.store(minimized, Ordering::Release);
  }

  pub(super) fn now(&self) -> Duration {
    self.clock.now()
  }

  /// The clock's time at the last beat; `None` before the first.
  pub(super) fn last(&self) -> Option<Duration> {
    match self.last.load(Ordering::Acquire) {
      0 => None,
      millis => Some(Duration::from_millis(millis - 1)),
    }
  }

  pub(super) fn minimized(&self) -> bool {
    self.minimized.load(Ordering::Acquire)
  }

  pub(super) fn screen(&self) -> &'static str {
    *self.screen.lock()
  }
}
