//! Counts the reads of project data the window starts on the test's thread, by what they read, from the app's
//! read log (`app_log::READ_LOG`): a screen starts its reads while it renders, on the window's thread.

use std::{collections::BTreeMap, fmt, sync::Arc};

use orchester_app::app_log::READ_LOG;
use parking_lot::Mutex;
use tracing::{
  Event, Subscriber,
  field::{Field, Visit},
  subscriber::DefaultGuard,
};
use tracing_subscriber::{
  Layer,
  layer::{Context, SubscriberExt},
};

/// The reads counted since the last [`Reads::take`], while it lives.
pub struct Reads {
  counted: Arc<Mutex<BTreeMap<String, usize>>>,
  _guard: DefaultGuard,
}

impl Reads {
  /// Counts the reads started on this thread from now on.
  pub fn count() -> Self {
    let counted = Arc::new(Mutex::new(BTreeMap::new()));
    let subscriber = tracing_subscriber::registry().with(Counter(Arc::clone(&counted)));
    Self {
      counted,
      _guard: tracing::subscriber::set_default(subscriber),
    }
  }

  /// The reads counted, by what they read, and counts again from none.
  pub fn take(&self) -> BTreeMap<String, usize> {
    std::mem::take(&mut *self.counted.lock())
  }

  /// How many reads were counted since the last [`Self::take`].
  pub fn total(&self) -> usize {
    self.counted.lock().values().sum()
  }
}

struct Counter(Arc<Mutex<BTreeMap<String, usize>>>);

impl<S: Subscriber> Layer<S> for Counter {
  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    if event.metadata().target() != READ_LOG {
      return;
    }
    let mut read = Read(None);
    event.record(&mut read);
    if let Some(read) = read.0 {
      *self.0.lock().entry(read).or_default() += 1;
    }
  }
}

/// The `read` field of a line.
struct Read(Option<String>);

impl Visit for Read {
  fn record_str(&mut self, field: &Field, value: &str) {
    if field.name() == "read" {
      self.0 = Some(value.to_owned());
    }
  }

  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    if field.name() == "read" {
      self.0 = Some(format!("{value:?}"));
    }
  }
}
