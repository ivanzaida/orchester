//! Holding a run's task at its last step, for requests that arrive once the run is recorded as
//! settled but before the task that settled it has ended. The task logs [`SETTLED`] as its last
//! step; the layer blocks the first task that does until the test lets it go, so the moment does
//! not depend on timing. It is installed as the test binary's only tracing subscriber, so a binary
//! holds one task in one test.

use std::{fmt, sync::mpsc as blocking};

use parking_lot::Mutex;
use tokio::sync::oneshot;
use tracing::{
  Event, Subscriber,
  field::{Field, Visit},
};
use tracing_subscriber::{
  layer::{Context, Layer, SubscriberExt},
  registry,
};

/// What a run's task logs as its last step, once it no longer executes the run.
const SETTLED: &str = "run settled";

/// The test's side of the hold.
pub struct Hold {
  /// Resolves once a task is held at its last step.
  pub held: oneshot::Receiver<()>,
  /// Lets the held task go.
  pub release: blocking::Sender<()>,
}

/// Installs the layer that holds the first task that logs [`SETTLED`].
pub fn hold_first_settled() -> Hold {
  let (held, is_held) = oneshot::channel();
  let (release, released) = blocking::channel();
  let layer = HoldSettled {
    held: Mutex::new(Some(held)),
    release: Mutex::new(Some(released)),
  };
  tracing::subscriber::set_global_default(registry().with(layer)).expect("the only subscriber of this test");
  Hold { held: is_held, release }
}

struct HoldSettled {
  held: Mutex<Option<oneshot::Sender<()>>>,
  release: Mutex<Option<blocking::Receiver<()>>>,
}

impl<S: Subscriber> Layer<S> for HoldSettled {
  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    let mut message = Message::default();
    event.record(&mut message);
    if message.0 != SETTLED {
      return;
    }
    let Some(release) = self.release.lock().take() else {
      return;
    };
    if let Some(held) = self.held.lock().take() {
      held.send(()).expect("the test waits for the task to be held");
    }
    release.recv().expect("the test lets the task go");
  }
}

#[derive(Default)]
struct Message(String);

impl Visit for Message {
  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    if field.name() == "message" {
      self.0 = format!("{value:?}");
    }
  }
}
