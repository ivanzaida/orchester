//! Counts the reads of project data the window starts on the test's thread, by what they read, from the app's
//! read log (`app_log::READ_LOG`): a screen starts its reads while it renders, on the window's thread. One
//! subscriber for the whole test binary counts by thread, so tests running side by side count apart; a
//! subscriber of the test's thread alone loses lines to the callsite cache another thread filled.

use std::{
  collections::{BTreeMap, HashMap},
  fmt,
  sync::{Arc, OnceLock},
  thread::{self, ThreadId},
};

use orchester_app::app_log::READ_LOG;
use parking_lot::Mutex;
use tracing::{
  Event, Subscriber,
  field::{Field, Visit},
};
use tracing_subscriber::{
  Layer,
  layer::{Context, SubscriberExt},
};

type Counted = Arc<Mutex<HashMap<ThreadId, BTreeMap<String, usize>>>>;

/// The reads counted, by thread, once the first test asked.
fn counted() -> &'static Counted {
  static COUNTED: OnceLock<Counted> = OnceLock::new();
  COUNTED.get_or_init(|| {
    let counted = Counted::default();
    let subscriber = tracing_subscriber::registry().with(Counter(Arc::clone(&counted)));
    tracing::subscriber::set_global_default(subscriber).expect("the only subscriber of the test binary");
    counted
  })
}

/// The reads counted on the test's thread since the last [`Reads::take`], while it lives.
pub struct Reads {
  thread: ThreadId,
}

impl Reads {
  /// Counts the reads started on this thread from now on.
  pub fn count() -> Self {
    let thread = thread::current().id();
    counted().lock().insert(thread, BTreeMap::new());
    Self { thread }
  }

  /// The reads counted, by what they read, and counts again from none.
  pub fn take(&self) -> BTreeMap<String, usize> {
    counted()
      .lock()
      .get_mut(&self.thread)
      .map(std::mem::take)
      .unwrap_or_default()
  }

  /// How many reads were counted since the last [`Self::take`].
  pub fn total(&self) -> usize {
    counted()
      .lock()
      .get(&self.thread)
      .map_or(0, |reads| reads.values().sum())
  }
}

impl Drop for Reads {
  fn drop(&mut self) {
    counted().lock().remove(&self.thread);
  }
}

struct Counter(Counted);

impl<S: Subscriber> Layer<S> for Counter {
  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    if event.metadata().target() != READ_LOG {
      return;
    }
    let mut read = Read(None);
    event.record(&mut read);
    let Some(read) = read.0 else {
      return;
    };
    // Only the threads of tests that count.
    if let Some(reads) = self.0.lock().get_mut(&thread::current().id()) {
      *reads.entry(read).or_default() += 1;
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
