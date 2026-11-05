//! Counts the reads of project data the window starts on the test's thread, by what they read, from the app's
//! read log (`app_log::READ_LOG`): a screen starts its reads while it renders, on the window's thread. One
//! subscriber for the whole test binary counts by thread, so tests running side by side count apart; a
//! subscriber of the test's thread alone loses lines to the callsite cache another thread filled.
//!
//! A settled read (`project::settled_read`) also writes an `ended` line, on whichever thread ran it, with the `id`
//! of its `read` line: by that id the read's end counts for the thread that started it, so a test can wait until
//! none of its reads still runs ([`Reads::running`]).

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

#[derive(Default)]
struct Tally {
  /// The reads each counting thread started since its last take, by what they read.
  started: HashMap<ThreadId, BTreeMap<String, usize>>,
  /// The settled reads a counting thread started that have not ended yet, by their `id`: the thread and what
  /// they read.
  running: HashMap<u64, (ThreadId, String)>,
}

type Counted = Arc<Mutex<Tally>>;

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
    counted().lock().started.insert(thread, BTreeMap::new());
    Self { thread }
  }

  /// The reads counted, by what they read, and counts again from none.
  pub fn take(&self) -> BTreeMap<String, usize> {
    counted()
      .lock()
      .started
      .get_mut(&self.thread)
      .map(std::mem::take)
      .unwrap_or_default()
  }

  /// How many reads were counted since the last [`Self::take`].
  pub fn total(&self) -> usize {
    counted()
      .lock()
      .started
      .get(&self.thread)
      .map_or(0, |reads| reads.values().sum())
  }

  /// What the settled reads this thread started that have not ended yet read, counted or taken.
  pub fn running(&self) -> Vec<String> {
    let mut running: Vec<String> = counted()
      .lock()
      .running
      .values()
      .filter(|(thread, _)| *thread == self.thread)
      .map(|(_, read)| read.clone())
      .collect();
    running.sort();
    running
  }
}

impl Drop for Reads {
  fn drop(&mut self) {
    let mut tally = counted().lock();
    tally.started.remove(&self.thread);
    tally.running.retain(|_, (thread, _)| *thread != self.thread);
  }
}

struct Counter(Counted);

impl<S: Subscriber> Layer<S> for Counter {
  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    if event.metadata().target() != READ_LOG {
      return;
    }
    let mut line = Line::default();
    event.record(&mut line);
    let mut tally = self.0.lock();
    if let Some(read) = line.read {
      // Only the threads of tests that count.
      let thread = thread::current().id();
      if let Some(reads) = tally.started.get_mut(&thread) {
        *reads.entry(read.clone()).or_default() += 1;
        if let Some(id) = line.id {
          tally.running.insert(id, (thread, read));
        }
      }
    } else if line.ended
      && let Some(id) = line.id
    {
      tally.running.remove(&id);
    }
  }
}

/// The fields of a line the count needs: `read` as a read starts, `ended` as a settled one ends, and its `id`.
#[derive(Default)]
struct Line {
  read: Option<String>,
  ended: bool,
  id: Option<u64>,
}

impl Visit for Line {
  fn record_u64(&mut self, field: &Field, value: u64) {
    if field.name() == "id" {
      self.id = Some(value);
    }
  }

  fn record_str(&mut self, field: &Field, value: &str) {
    match field.name() {
      "read" => self.read = Some(value.to_owned()),
      "ended" => self.ended = true,
      _ => {}
    }
  }

  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    match field.name() {
      "read" => self.read = Some(format!("{value:?}")),
      "ended" => self.ended = true,
      _ => {}
    }
  }
}
