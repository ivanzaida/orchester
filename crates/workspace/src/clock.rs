//! The time the window shows live values at (a run's active agent time while an agent works),
//! shared with screens through the UI context. The application reads the system clock; tests hand in
//! a clock they set themselves.

use std::sync::Arc;

use orchester_core::time::Timestamp;

/// Tells the time.
pub trait TimeSource: Send + Sync {
  fn now(&self) -> Timestamp;
}

struct SystemTime;

impl TimeSource for SystemTime {
  fn now(&self) -> Timestamp {
    Timestamp::now()
  }
}

#[derive(Clone)]
pub struct Clock(Arc<dyn TimeSource>);

impl Clock {
  /// The computer's clock.
  pub fn system() -> Self {
    Self(Arc::new(SystemTime))
  }

  /// The time `source` tells, e.g. a test's.
  pub fn with(source: Arc<dyn TimeSource>) -> Self {
    Self(source)
  }

  pub fn now(&self) -> Timestamp {
    self.0.now()
  }
}

impl PartialEq for Clock {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}
