//! The transport of a client whose reports are turned on and off while it runs ([`Switch::set`], feature `switch`):
//! the runner's, which the app turns, and the app's own, which the person's setting turns. Off, every envelope still
//! waiting to leave is dropped; on again, a new transport sends from then on. Each program keeps its one client bound
//! for as long as it runs, because each thread keeps the client that was bound when it first reported, and asks
//! [`Switch::is_on`] or [`Switch::with`] in its `before_send`, so no report is even made while they are off.
//!
//! Sentry's HTTP transport keeps its own queue, which only its drop empties. So the switch keeps the waiting
//! envelopes in a queue of its own and hands the transport one at a time: turning the reports off clears the queue's
//! [`On::live`] at once, and the queue drops the rest instead of handing them on.

use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use parking_lot::RwLock;
use sentry::{
  ClientOptions, Envelope, Transport, TransportFactory, TransportOptions,
  transports::{TokioTransportThread, TokioTransportThreadOptions},
};

/// How long the queue waits for the transport to send one envelope before it hands it the next.
const SEND_TIMEOUT: Duration = Duration::from_secs(30);

/// Turns a client's reports on with a `T` (what they are sent as: the runner's environment and release) and off;
/// every clone switches the same client.
pub struct Switch<T>(Arc<Switched<T>>);

impl<T> Clone for Switch<T> {
  fn clone(&self) -> Self {
    Self(Arc::clone(&self.0))
  }
}

struct Switched<T> {
  transports: Arc<dyn TransportFactory>,
  /// The DSN and the proxies each transport is made with.
  options: ClientOptions,
  on: RwLock<Option<On<T>>>,
}

/// Reports while they are on: what they are sent as, and the queue that hands them to their transport.
struct On<T> {
  value: T,
  /// Cleared when the reports are turned off; the queue then hands nothing on.
  live: Arc<AtomicBool>,
  queue: Arc<TokioTransportThread>,
}

impl<T: Send + Sync + 'static> Switch<T> {
  /// Off until [`Self::set`] turns it on; each transport is made by `transports` with `options`.
  pub fn new(transports: Arc<dyn TransportFactory>, options: ClientOptions) -> Self {
    Self(Arc::new(Switched {
      transports,
      options,
      on: RwLock::new(None),
    }))
  }

  /// Sends the reports made from now on as `value`, or nothing with `None`. Turning them off also drops the reports
  /// waiting to leave: only one already on its way still arrives.
  pub fn set(&self, value: Option<T>) {
    let mut on = self.0.on.write();
    match (value, on.as_mut()) {
      (Some(value), Some(current)) => current.value = value,
      (Some(value), None) => *on = Some(self.0.start(value)),
      (None, _) => {
        if let Some(off) = on.take() {
          off.live.store(false, Ordering::Release);
          drop(on);
          discard(off.queue);
        }
      }
    }
  }

  pub fn is_on(&self) -> bool {
    self.0.on.read().is_some()
  }

  /// `read` of what the reports are sent as while they are on; `None` while they are off.
  pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> Option<R> {
    self.0.on.read().as_ref().map(|on| read(&on.value))
  }
}

impl<T> Switched<T> {
  /// A new transport for `value`, behind a queue that hands it each envelope while they stay on.
  fn start(&self, value: T) -> On<T> {
    let options = TransportOptions::try_from_client_options(&self.options)
      .expect("the reports' options always name GlitchTip's DSN (`client::DSN`)");
    let transport = self.transports.create_transport_with_options(options);
    let live = Arc::new(AtomicBool::new(true));
    let handing = live.clone();
    let queue = TokioTransportThreadOptions::new(move |envelope: Envelope, limits| {
      if handing.load(Ordering::Acquire) {
        transport.send_envelope(envelope);
        // One at a time, so the rest wait here, where turning the reports off drops them.
        transport.flush(SEND_TIMEOUT);
      }
      std::future::ready(limits)
    })
    .spawn_thread();
    On {
      value,
      live,
      queue: Arc::new(queue),
    }
  }

  /// The queue of the moment, held only while it is asked, so switching off never waits for a flush.
  fn current(&self) -> Option<Arc<TokioTransportThread>> {
    self.on.read().as_ref().map(|on| on.queue.clone())
  }
}

/// Ends `queue` and its transport. Both wait for an envelope already on its way, so that waiting happens on a thread
/// of its own.
fn discard(queue: Arc<TokioTransportThread>) {
  let ending = std::thread::Builder::new()
    .name("error-reports-off".into())
    .spawn(move || drop(queue));
  if let Err(error) = ending {
    // The closure, and with it the queue, was dropped here already.
    tracing::warn!(%error, "ended the error reports' transport on the turning thread");
  }
}

/// The client asks the switch for its one transport, which then hands each envelope to the queue of the moment.
impl<T: Send + Sync + 'static> TransportFactory for Switch<T> {
  fn create_transport_with_options(&self, _: TransportOptions) -> Arc<dyn Transport> {
    self.0.clone()
  }
}

impl<T: Send + Sync + 'static> Transport for Switched<T> {
  fn send_envelope(&self, envelope: Envelope) {
    if let Some(on) = self.on.read().as_ref() {
      on.queue.send(envelope);
    }
  }

  fn flush(&self, timeout: Duration) -> bool {
    self.current().is_none_or(|queue| queue.flush(timeout))
  }

  fn shutdown(&self, timeout: Duration) -> bool {
    self.flush(timeout)
  }
}
