//! The runner's error reports (ORC-158, ORC-159): with reports on, a panic is one scrubbed event with
//! `process=runner` and the app's environment and release, and so is an ERROR log line, while a WARN line sends
//! nothing; a debug build starts nothing. Turned off, the reports still waiting are dropped and none is sent until
//! they are turned on again.
//!
//! `cargo test -p orchester-runner --test error_reports error_reports_`

use std::{
  panic,
  sync::{Arc, Mutex, mpsc},
  time::Duration,
};

use orchester_ipc::ErrorReports;
use orchester_runner::error_reports::{self, PROCESS, Reports};
use sentry::{
  Envelope, Hub, Scope, Transport, TransportFactory, TransportOptions,
  protocol::Event,
  test::with_captured_events_options,
  transports::{TokioTransportThread, TokioTransportThreadOptions},
};
use tracing_subscriber::layer::SubscriberExt;

const RELEASE: &str = "orchester@9.8.7";
const PATIENCE: Duration = Duration::from_secs(10);

fn reports() -> ErrorReports {
  ErrorReports::new("qa", RELEASE).expect("both are text")
}

/// The events the runner's options report while `body` logs through the runner's error report layer.
fn reported(body: impl FnOnce()) -> Vec<Event<'static>> {
  let subscriber = tracing_subscriber::registry().with(error_reports::layer());
  with_captured_events_options(
    || tracing::subscriber::with_default(subscriber, body),
    error_reports::options(&reports()),
  )
}

#[test]
fn error_reports_a_panic_is_one_scrubbed_event_tagged_runner() {
  let home = std::env::home_dir().expect("the test runs with a home folder");
  let private = home.join("acme-secret").join("notes.txt");
  let message = format!("cannot read {} with sk-ant-api03-AbCdEf123456GhIjKl", private.display());

  let events = reported(|| {
    let panicked = panic::catch_unwind(|| panic!("{message}"));
    assert!(panicked.is_err(), "the closure panics");
  });

  assert_eq!(events.len(), 1, "{events:?}");
  let event = &events[0];
  assert_eq!(event.tags.get("process").map(String::as_str), Some(PROCESS));
  assert_eq!(PROCESS, "runner");
  assert_eq!(event.release.as_deref(), Some(RELEASE));
  assert_eq!(event.environment.as_deref(), Some("qa"));
  assert_eq!(event.level, sentry::Level::Fatal);
  let exception = &event.exception.values[0];
  assert_eq!(exception.ty, "panic");
  let value = exception.value.as_deref().expect("the panic's message stays");
  assert!(value.starts_with("cannot read ~"), "{value}");
  assert!(value.ends_with("with <redacted>"), "{value}");
  let sent = format!("{event:?}");
  for removed in ["acme-secret", "sk-ant-api03"] {
    assert!(!sent.contains(removed), "{removed} is left in {sent}");
  }
  assert_eq!(event.server_name, None);
  assert_eq!(event.user, None);
  assert!(event.extra.is_empty() && event.breadcrumbs.values.is_empty());
}

#[test]
fn error_reports_an_error_line_is_one_scrubbed_event_and_a_warning_none() {
  let home = std::env::home_dir().expect("the test runs with a home folder");
  let private = home.join("acme-secret").join("worktree");

  let events = reported(|| {
    tracing::warn!(path = %private.display(), "a warning is no report");
    tracing::error!(error = %format!("cannot remove {}", private.display()), "cannot remove the worktree");
  });

  assert_eq!(events.len(), 1, "{events:?}");
  let event = &events[0];
  assert_eq!(event.level, sentry::Level::Error);
  assert_eq!(event.tags.get("process").map(String::as_str), Some("runner"));
  let message = event.message.as_deref().expect("the line's message and fields");
  assert!(
    message.starts_with("cannot remove the worktree error=cannot remove ~"),
    "{message}"
  );
  let sent = format!("{event:?}");
  for removed in ["acme-secret", "a warning", "Rust Tracing"] {
    assert!(!sent.contains(removed), "{removed} is left in {sent}");
  }
}

#[test]
fn error_reports_in_a_debug_build_nothing_starts() {
  let options = error_reports::options(&reports());
  assert!(options.before_send.is_some() && !options.send_default_pii);

  assert!(Reports::start().is_none(), "a test build never reports");
  assert!(Hub::main().client().is_none(), "nothing in the tests started a client");
}

/// What the transports of [`Transports`] sent: each event's message and release, in order.
type Sent = Arc<Mutex<Vec<(String, String)>>>;

/// Makes the transports of the runner's client, the way sentry's HTTP transport sends: from a queue on a thread of
/// its own. The first one holds its first report on its way until the test lets it go, so the reports after it wait.
struct Transports {
  sent: Sent,
  made: Mutex<usize>,
  /// The held report is on its way.
  sending: Mutex<Option<mpsc::Sender<()>>>,
  /// Lets the held report arrive.
  release: Mutex<Option<mpsc::Receiver<()>>>,
  /// The first transport ended.
  ended: Mutex<Option<mpsc::Sender<()>>>,
}

impl TransportFactory for Transports {
  fn create_transport_with_options(&self, _: TransportOptions) -> Arc<dyn Transport> {
    *self.made.lock().expect("count") += 1;
    let (sending, release) = (
      self.sending.lock().expect("sending").take(),
      self.release.lock().expect("release").take(),
    );
    let mut hold = sending.zip(release);
    let sent = self.sent.clone();
    let thread = TokioTransportThreadOptions::new(move |envelope: Envelope, limits| {
      if let Some(event) = envelope.event() {
        let message = event.message.clone().unwrap_or_default();
        let release = event.release.as_deref().unwrap_or_default().to_owned();
        sent.lock().expect("sent").push((message, release));
      }
      if let Some((sending, release)) = hold.take() {
        sending.send(()).expect("the test waits for the held report");
        release.recv().expect("the test lets the held report go");
      }
      std::future::ready(limits)
    })
    .spawn_thread();
    Arc::new(Queued {
      thread: Some(thread),
      ended: self.ended.lock().expect("ended").take(),
    })
  }
}

/// A transport of [`Transports`]; dropping it ends its thread, which skips what still waits in its queue.
struct Queued {
  thread: Option<TokioTransportThread>,
  ended: Option<mpsc::Sender<()>>,
}

impl Transport for Queued {
  fn send_envelope(&self, envelope: Envelope) {
    self.thread.as_ref().expect("until dropped").send(envelope);
  }

  fn flush(&self, timeout: Duration) -> bool {
    self.thread.as_ref().expect("until dropped").flush(timeout)
  }
}

impl Drop for Queued {
  fn drop(&mut self) {
    drop(self.thread.take());
    if let Some(ended) = self.ended.take() {
      // The test may have stopped waiting for it.
      let _ = ended.send(());
    }
  }
}

#[test]
fn error_reports_off_drops_the_waiting_reports_and_sends_none_until_turned_on_again() {
  let (sending, on_its_way) = mpsc::channel();
  let (release, let_go) = mpsc::channel();
  let (ended, first_ended) = mpsc::channel();
  let sent = Sent::default();
  let transports = Arc::new(Transports {
    sent: sent.clone(),
    made: Mutex::new(0),
    sending: Mutex::new(Some(sending)),
    release: Mutex::new(Some(let_go)),
    ended: Mutex::new(Some(ended)),
  });
  let runner = Reports::new(Some(reports()), transports.clone());
  let hub = Hub::new(Some(runner.client().clone()), Arc::new(Scope::default()));
  let switch = runner.switch();

  hub.capture_message("on its way", sentry::Level::Error);
  on_its_way
    .recv_timeout(PATIENCE)
    .expect("the first report is on its way");
  hub.capture_message("waiting one", sentry::Level::Error);
  hub.capture_message("waiting two", sentry::Level::Error);
  switch.set(None);
  hub.capture_message("while off", sentry::Level::Error);
  release.send(()).expect("the held report goes");
  first_ended
    .recv_timeout(PATIENCE)
    .expect("turning off ends the transport");
  assert_eq!(
    *sent.lock().expect("sent"),
    [("on its way".to_owned(), RELEASE.to_owned())],
    "only the report already on its way arrives"
  );

  let later = ErrorReports::new("production", "orchester@9.8.8").expect("both are text");
  switch.set(Some(later));
  hub.capture_message("on again", sentry::Level::Error);
  assert!(runner.client().flush(Some(PATIENCE)), "the report leaves");
  assert_eq!(
    sent.lock().expect("sent").last(),
    Some(&("on again".to_owned(), "orchester@9.8.8".to_owned())),
    "on again sends with the release it was turned on with"
  );
  assert_eq!(sent.lock().expect("sent").len(), 2);
  assert_eq!(
    *transports.made.lock().expect("count"),
    2,
    "on again sends through a new transport"
  );
}

#[test]
fn error_reports_started_off_send_nothing_until_turned_on() {
  let sent = Sent::default();
  let transports = Arc::new(Transports {
    sent: sent.clone(),
    made: Mutex::new(0),
    sending: Mutex::new(None),
    release: Mutex::new(None),
    ended: Mutex::new(None),
  });
  let runner = Reports::new(None, transports.clone());
  let hub = Hub::new(Some(runner.client().clone()), Arc::new(Scope::default()));

  hub.capture_message("while off", sentry::Level::Error);
  assert!(runner.client().flush(Some(PATIENCE)));
  assert!(sent.lock().expect("sent").is_empty(), "off sends nothing");
  assert_eq!(*transports.made.lock().expect("count"), 0, "off makes no transport");

  runner.switch().set(Some(reports()));
  hub.capture_message("turned on", sentry::Level::Error);
  assert!(runner.client().flush(Some(PATIENCE)), "the report leaves");
  assert_eq!(
    *sent.lock().expect("sent"),
    [("turned on".to_owned(), RELEASE.to_owned())]
  );
}
