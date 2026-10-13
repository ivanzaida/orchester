//! Holds cleanup just before it stops workers, so requests arrive during failure handling
//! without sleeps or a race with the failed worker's end. One test binary installs this layer.

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

pub struct Hold {
  pub held: oneshot::Receiver<()>,
  pub release: blocking::Sender<()>,
}

pub fn hold_cleanup() -> Hold {
  let (held, is_held) = oneshot::channel();
  let (release, released) = blocking::channel();
  let layer = FailureHold {
    channels: Mutex::new(Some((held, released))),
  };
  tracing::subscriber::set_global_default(registry().with(layer)).expect("only subscriber of this binary");
  Hold { held: is_held, release }
}

struct FailureHold {
  channels: Mutex<Option<(oneshot::Sender<()>, blocking::Receiver<()>)>>,
}

impl<S: Subscriber> Layer<S> for FailureHold {
  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    let mut message = Message::default();
    event.record(&mut message);
    if message.0 != "ending workers after runner failure" {
      return;
    }
    let Some((held, release)) = self.channels.lock().take() else {
      return;
    };
    held.send(()).expect("test waits for cleanup");
    release
      .recv_timeout(super::PATIENCE)
      .expect("test releases cleanup in time");
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
