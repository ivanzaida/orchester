//! The app's error reports (ORC-156, ORC-157, ORC-162, ORC-161): a panic becomes one scrubbed event with the release
//! and the environment, and so does an ERROR log line, while a WARN line sends nothing; a panic names the loaded
//! modules with their debug IDs and its frames' instruction addresses; debug builds and tests never start the client;
//! the client follows the person's setting, sending nothing while it is off and sending again once it is turned on.

use std::{
  ffi::OsStr,
  panic,
  sync::{Arc, Mutex},
  time::Duration,
};

use orchester_app::{
  app_log::{self, PANIC_LOG},
  error_reports::{self, Environment, RELEASE, Reports},
  mcp::Build,
};
use orchester_store::AppDb;
use orchester_workspace::workspace::Workspace;
use sentry::{
  Hub, Scope,
  protocol::{DebugImage, Event},
  test::{TestTransport, with_captured_events_options},
};
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
  assert_eq!(event.tags.get("process").map(String::as_str), Some("app"));
}

#[test]
fn error_reports_the_runners_report_in_the_apps_environment_and_release() {
  let reports = error_reports::for_runners(Environment::Production);
  assert_eq!(reports.environment(), "production");
  assert_eq!(reports.release(), RELEASE);
}

/// GlitchTip resolves a release build's raw frames in its uploaded debug files: the event names this program's image
/// with its debug ID (a PE's, with its PDB, on Windows), and its frames carry instruction addresses inside it.
#[test]
fn error_reports_a_panic_names_its_debug_images_and_instruction_addresses() {
  let events = with_captured_events_options(
    || {
      let panicked = panic::catch_unwind(|| panic!("index out of bounds"));
      assert!(panicked.is_err(), "the closure panics");
    },
    error_reports::options(Environment::Qa),
  );

  assert_eq!(events.len(), 1, "{events:?}");
  let event = &events[0];
  let program = std::env::current_exe().expect("the test's program");
  let program = program.file_name().and_then(OsStr::to_str).expect("a file name");
  let image = event
    .debug_meta
    .images
    .iter()
    .find_map(|image| match image {
      DebugImage::Symbolic(image) if image.name == program => Some(image),
      _ => None,
    })
    .unwrap_or_else(|| panic!("{program} is among the debug images: {:?}", event.debug_meta));
  assert!(!image.id.is_nil(), "{image:?}");
  if cfg!(windows) {
    assert!(image.code_id.is_some(), "{image:?}");
    let pdb = image.debug_file.as_deref().expect("a PE names its PDB");
    assert!(pdb.ends_with(".pdb") && !pdb.contains(['/', '\\']), "{pdb}");
  }

  let start = image.image_addr.0;
  let end = start + image.image_size;
  let frames = &event.exception.values[0]
    .stacktrace
    .as_ref()
    .expect("the panic's stack trace")
    .frames;
  assert!(
    frames
      .iter()
      .filter_map(|frame| frame.instruction_addr)
      .any(|address| (start..end).contains(&address.0)),
    "a frame lies in {program}: {frames:?}"
  );
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
  assert!(
    !error_reports::turned_off(),
    "with no client of the app, nothing is turned off"
  );
}

#[test]
fn error_reports_follow_the_stored_setting() {
  let (_off_dir, off) = workspace(false);
  assert_eq!(error_reports::allowed(Some(&off)), Some(false));
  assert_eq!(error_reports::allowed(None), None, "no session, no setting");
  assert!(
    error_reports::start(Environment::Production, None).is_none(),
    "no session, no client"
  );
  assert!(Hub::current().client().is_none() && Hub::main().client().is_none());

  let (_on_dir, on) = workspace(true);
  assert_eq!(error_reports::allowed(Some(&on)), Some(true));
  on.set_sends_error_reports(false).expect("the person's choice is kept");
  assert_eq!(error_reports::allowed(Some(&on)), Some(false), "turned off is kept");
  let options = error_reports::options(Environment::Production);
  assert_eq!(options.environment.as_deref(), Some("production"));
  assert!(options.before_send.is_some() && !options.send_default_pii);
}

/// How long a test waits for its reports to leave.
const PATIENCE: Duration = Duration::from_secs(10);

/// The messages `transport` received, in order.
fn sent(transport: &TestTransport) -> Vec<String> {
  transport
    .fetch_and_clear_events()
    .into_iter()
    .map(|event| event.message.unwrap_or_default())
    .collect()
}

#[test]
fn error_reports_turned_off_the_client_sends_nothing_and_turned_on_again_it_sends() {
  let transport = TestTransport::new();
  let reports = Reports::new(Environment::Qa, true, Arc::new(transport.clone()));
  let hub = Arc::new(Hub::new(Some(reports.client().clone()), Arc::new(Scope::default())));
  let switch = reports.switch();

  hub.capture_message("while on", sentry::Level::Error);
  assert!(reports.client().flush(Some(PATIENCE)), "the report leaves");
  assert_eq!(sent(&transport), ["while on"]);

  switch.set(None);
  assert!(reports.for_runners().is_none(), "the runners are told to stop too");
  hub.capture_message("while off", sentry::Level::Error);
  Hub::run(hub.clone(), || {
    let panicked = panic::catch_unwind(|| panic!("a panic while off"));
    assert!(panicked.is_err(), "the closure panics");
  });
  assert!(reports.client().flush(Some(PATIENCE)));
  assert!(sent(&transport).is_empty(), "off sends nothing");

  switch.set(Some(()));
  hub.capture_message("on again", sentry::Level::Error);
  assert!(reports.client().flush(Some(PATIENCE)), "the report leaves");
  let events = transport.fetch_and_clear_events();
  assert_eq!(events.len(), 1, "{events:?}");
  assert_eq!(events[0].message.as_deref(), Some("on again"));
  assert_eq!(events[0].release.as_deref(), Some(RELEASE));
  assert_eq!(events[0].environment.as_deref(), Some("qa"));
  assert_eq!(events[0].tags.get("process").map(String::as_str), Some("app"));
  assert_eq!(
    reports.for_runners(),
    Some(error_reports::for_runners(Environment::Qa)),
    "the runners are told to send again"
  );
}

#[test]
fn error_reports_started_off_send_nothing_until_turned_on_without_a_restart() {
  let transport = TestTransport::new();
  let reports = Reports::new(Environment::Production, false, Arc::new(transport.clone()));
  let hub = Hub::new(Some(reports.client().clone()), Arc::new(Scope::default()));

  hub.capture_message("while off", sentry::Level::Error);
  assert!(reports.client().flush(Some(PATIENCE)));
  assert!(sent(&transport).is_empty(), "off sends nothing");
  assert!(reports.for_runners().is_none());

  reports.switch().set(Some(()));
  hub.capture_message("turned on", sentry::Level::Error);
  assert!(reports.client().flush(Some(PATIENCE)), "the report leaves");
  assert_eq!(sent(&transport), ["turned on"]);
}
