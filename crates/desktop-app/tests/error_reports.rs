//! The app's error reports (ORC-156): a panic becomes one scrubbed event with the release and the environment; debug
//! builds and tests never start the client, and nothing starts while the person's setting is off.

use std::{ffi::OsStr, panic};

use orchester_app::{
  error_reports::{self, Environment, RELEASE},
  mcp::Build,
};
use orchester_store::AppDb;
use orchester_workspace::workspace::Workspace;
use sentry::{Hub, test::with_captured_events_options};

fn workspace(sends: bool) -> (tempfile::TempDir, Workspace) {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("the application database");
  db.set_sends_error_reports(sends).expect("the setting is kept");
  (dir, Workspace::new(db))
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
