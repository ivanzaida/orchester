//! The app's error reports (ORC-156, ORC-157): a panic becomes one scrubbed event with the release and the
//! environment, and so does an ERROR log line, while a WARN line sends nothing; debug builds and tests never start the
//! client, and nothing starts while the person's setting is off.

use std::{
  ffi::OsStr,
  panic,
  sync::{Arc, Mutex},
};

use orchester_app::{
  app_log::{self, PANIC_LOG},
  error_reports::{self, Environment, RELEASE},
  mcp::Build,
};
use orchester_store::AppDb;
use orchester_workspace::workspace::Workspace;
use sentry::{Hub, protocol::Event, test::with_captured_events_options};
use tracing::{Level, Subscriber};
use tracing_subscriber::{
  Layer,
  layer::{Context, SubscriberExt},
};

fn workspace(sends: bool) -> (tempfile::TempDir, Workspace) {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("the application database");
  db.set_sends_error_reports(sends).expect("the setting is kept");
  (dir, Workspace::new(db))
}

/// The targets of the ERROR lines logged, so a test sees a line was logged that sent no report.
#[derive(Clone, Default)]
struct ErrorLines(Arc<Mutex<Vec<String>>>);

impl<S: Subscriber> Layer<S> for ErrorLines {
  fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
    if *event.metadata().level() == Level::ERROR {
      let mut lines = self.0.lock().expect("no test panics while holding the lines");
      lines.push(event.metadata().target().to_owned());
    }
  }
}

/// The events the app's options report while `body` logs through the app's error report layer.
fn reported(lines: &ErrorLines, body: impl FnOnce()) -> Vec<Event<'static>> {
  let subscriber = tracing_subscriber::registry()
    .with(app_log::error_reports())
    .with(lines.clone());
  with_captured_events_options(
    || tracing::subscriber::with_default(subscriber, body),
    error_reports::options(Environment::Qa),
  )
}

#[test]
fn error_reports_an_error_line_is_one_scrubbed_event_and_a_warning_none() {
  let home = std::env::home_dir().expect("the test runs with a home folder");
  let private = home.join("acme-secret").join("notes.txt");
  let lines = ErrorLines::default();

  let events = reported(&lines, || {
    tracing::warn!(path = %private.display(), "a warning is no report");
    tracing::info!("nor is an info line");
    tracing::error!(
      error = %format!("cannot read {}", private.display()),
      key = "sk-ant-api03-AbCdEf123456GhIjKl",
      "cannot save the task"
    );
  });

  assert_eq!(events.len(), 1, "{events:?}");
  let event = &events[0];
  assert_eq!(event.level, sentry::Level::Error);
  assert_eq!(event.release.as_deref(), Some(RELEASE));
  assert_eq!(event.environment.as_deref(), Some("qa"));
  let message = event.message.as_deref().expect("the line's message and fields");
  assert!(
    message.starts_with("cannot save the task error=cannot read ~"),
    "{message}"
  );
  assert!(message.ends_with("key=<redacted>"), "{message}");
  let sent = format!("{event:?}");
  for removed in ["acme-secret", "sk-ant-api03", "a warning", "Rust Tracing"] {
    assert!(!sent.contains(removed), "{removed} is left in {sent}");
  }
  assert!(event.breadcrumbs.values.is_empty() && event.extra.is_empty());
  assert!(
    event.threads.values.iter().any(|thread| thread.stacktrace.is_some()),
    "the stack where the line was logged: {event:?}"
  );
}

#[test]
fn error_reports_a_panic_logged_at_error_is_still_one_event() {
  app_log::log_panics();
  let lines = ErrorLines::default();

  let events = reported(&lines, || {
    let panicked = panic::catch_unwind(|| panic!("the window's state is broken"));
    assert!(panicked.is_err(), "the closure panics");
  });

  let logged = lines.0.lock().expect("the test holds the lines").clone();
  assert_eq!(logged, [PANIC_LOG], "the panic hook logs the panic at ERROR");
  assert_eq!(events.len(), 1, "{events:?}");
  assert_eq!(events[0].level, sentry::Level::Fatal);
  assert_eq!(events[0].exception.values[0].ty, "panic");
}

#[test]
fn error_reports_a_panic_is_one_scrubbed_event_with_release_and_environment() {
  let home = std::env::home_dir().expect("the test runs with a home folder");
  let private = home.join("acme-secret").join("notes.txt");
  let message = format!("cannot read {} with sk-ant-api03-AbCdEf123456GhIjKl", private.display());

  let events = with_captured_events_options(
    || {
      let panicked = panic::catch_unwind(|| panic!("{message}"));
      assert!(panicked.is_err(), "the closure panics");
    },
    error_reports::options(Environment::Qa),
  );

  assert_eq!(events.len(), 1, "{events:?}");
  let event = &events[0];
  assert_eq!(event.release.as_deref(), Some(RELEASE));
  assert_eq!(RELEASE, concat!("orchester@", env!("CARGO_PKG_VERSION")));
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
fn error_reports_never_start_in_debug_builds_or_tests() {
  let qa = Some(OsStr::new("mcp"));
  assert_eq!(Environment::of_this_app(), None, "a test build never reports");
  assert_eq!(Environment::of_app(Build::of(Some("1"), true), true, None), None);
  assert_eq!(Environment::of_app(Build::Other, true, qa), None);
  assert_eq!(Environment::of_app(Build::Other, false, None), None);
  assert_eq!(Environment::of_app(Build::Other, false, Some(OsStr::new(""))), None);
  assert_eq!(Environment::of_app(Build::Other, false, qa), Some(Environment::Qa));
  assert_eq!(
    Environment::of_app(Build::of(Some("1"), false), false, None),
    Some(Environment::Production)
  );
  assert!(Hub::main().client().is_none(), "nothing in the tests started a client");
}

#[test]
fn error_reports_with_the_setting_off_nothing_starts() {
  let (_off_dir, off) = workspace(false);
  assert!(error_reports::allowed_options(Environment::Qa, Some(&off)).is_none());
  assert!(error_reports::start(Environment::Qa, Some(&off)).is_none());
  assert!(
    error_reports::start(Environment::Production, None).is_none(),
    "no session, no setting"
  );
  assert!(Hub::current().client().is_none() && Hub::main().client().is_none());

  let (_on_dir, on) = workspace(true);
  let options = error_reports::allowed_options(Environment::Production, Some(&on)).expect("on allows reports");
  assert_eq!(options.environment.as_deref(), Some("production"));
  assert!(options.before_send.is_some() && !options.send_default_pii);
}
