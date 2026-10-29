//! The runner's error reports (ORC-158): started with reports on, a panic is one scrubbed event with `process=runner`
//! and the app's environment and release, and so is an ERROR log line, while a WARN line sends nothing; started
//! without, or as a debug build, nothing starts.
//!
//! `cargo test -p orchester-runner --test error_reports error_reports_`

use std::panic;

use orchester_ipc::ErrorReports;
use orchester_runner::error_reports::{self, PROCESS};
use sentry::{Hub, protocol::Event, test::with_captured_events_options};
use tracing_subscriber::layer::SubscriberExt;

const RELEASE: &str = "orchester@9.8.7";

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
fn error_reports_without_the_app_saying_so_or_in_a_debug_build_nothing_starts() {
  assert!(
    error_reports::allowed_options(None, false).is_none(),
    "started without reports"
  );
  assert!(
    error_reports::allowed_options(Some(&reports()), true).is_none(),
    "a debug build never reports"
  );
  let options = error_reports::allowed_options(Some(&reports()), false).expect("a release build started with reports");
  assert_eq!(options.environment.as_deref(), Some("qa"));
  assert_eq!(options.release.as_deref(), Some(RELEASE));
  assert!(options.before_send.is_some() && !options.send_default_pii);

  assert!(error_reports::start().is_none(), "a test build never reports");
  assert!(Hub::main().client().is_none(), "nothing in the tests started a client");
}
