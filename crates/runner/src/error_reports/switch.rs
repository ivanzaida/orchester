//! The transport of the runner's client, which the app turns on and off ([`Switch::set`]). Off, every report is
//! dropped before it is made into an envelope, and so is every envelope still waiting to leave; on again, a new
//! transport sends from then on.
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

use orchester_ipc::ErrorReports;
use parking_lot::RwLock;
use sentry::{
  ClientOptions, Envelope, Transport, TransportFactory, TransportOptions,
  protocol::Event,
  transports::{TokioTransportThread, TokioTransportThreadOptions},
};

/// How long the queue waits for the transport to send one envelope before it hands it the next.
const SEND_TIMEOUT: Duration = Duration::from_secs(30);

/// Turns the runner's reports on and off; every clone switches the same client.
#[derive(Clone)]
pub struct Switch(Arc<Switched>);

struct Switched {
  transports: Arc<dyn TransportFactory>,
  /// The DSN and the proxies each transport is made with.
  options: ClientOptions,
  on: RwLock<Option<On>>,
}

/// Reports while they are on: whose they are, and the queue that hands them to their transport.
struct On {
  reports: ErrorReports,
  /// Cleared when the reports are turned off; the queue then hands nothing on.
  live: Arc<AtomicBool>,
  queue: Arc<TokioTransportThread>,
}

impl Switch {
  /// Off until [`Self::set`] turns it on; each transport is made by `transports` with `options`.
  pub(super) fn new(transports: Arc<dyn TransportFactory>, options: ClientOptions) -> Self {
    Self(Arc::new(Switched {
      transports,
      options,
      on: RwLock::new(None),
    }))
  }

  /// Sends the reports made from now on with `reports`, or nothing with `None`. Turning them off also drops the
  /// reports waiting to leave: only one already on its way still arrives.
  pub fn set(&self, reports: Option<ErrorReports>) {
    let mut on = self.0.on.write();
    match (reports, on.as_mut()) {
      (Some(reports), Some(current)) => current.reports = reports,
      (Some(reports), None) => *on = Some(self.0.start(reports)),
      (None, _) => {
        if let Some(off) = on.take() {
          off.live.store(false, Ordering::Release);
          drop(on);
          discard(off.queue);
        }
      }
    }
  }

  /// `event` with the release and the environment of the reports while they are on; `None` while they are off.
  pub(super) fn stamp(&self, mut event: Event<'static>) -> Option<Event<'static>> {
    let on = self.0.on.read();
    let reports = &on.as_ref()?.reports;
    event.release = Some(reports.release().to_owned().into());
    event.environment = Some(reports.environment().to_owned().into());
    Some(event)
  }
}

impl Switched {
  /// A new transport for `reports`, behind a queue that hands it each envelope while they stay on.
  fn start(&self, reports: ErrorReports) -> On {
    let options = TransportOptions::try_from_client_options(&self.options)
      .expect("the reports' options always name GlitchTip's DSN (`orchester_report::client::DSN`)");
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
      reports,
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
    tracing::warn!(%error, "ended the error reports' transport on the runner's own thread");
  }
}

/// The client asks the switch for its one transport, which then hands each envelope to the queue of the moment.
impl TransportFactory for Switch {
  fn create_transport_with_options(&self, _: TransportOptions) -> Arc<dyn Transport> {
    self.0.clone()
  }
}

impl Transport for Switched {
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
