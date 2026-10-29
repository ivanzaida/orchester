//! The runner's panics and ERROR log lines as error reports to GlitchTip (`docs/error-reports.md`), the way the app
//! sends its own: the same project, client options and scrubber (`orchester_report::client`), with the tag
//! `process=runner`.
//!
//! The runner never reads the person's setting: it reports only when the app that started it says so
//! (`orchester_ipc::error_reports`), in the environment and release the app names, and keeps that until it exits.
//! Debug builds, the tests among them, never report. Its log file and stderr stay as they are.

use orchester_ipc::ErrorReports;
use sentry::{ClientInitGuard, ClientOptions, integrations::tracing::ErrorLayer};
use tracing::{Level, Subscriber};
use tracing_subscriber::{Layer, filter::filter_fn, registry::LookupSpan};

/// Which of Orchester's programs a report comes from (the `process` tag the scrubber keeps).
pub const PROCESS: &str = "runner";

/// The client's options for `reports`: GlitchTip's `desktop` project, the app's environment and release, the tag
/// `process=runner`, and the scrubber as `before_send`.
pub fn options(reports: &ErrorReports) -> ClientOptions {
  orchester_report::client::options(
    PROCESS,
    reports.release().to_owned(),
    reports.environment().to_owned(),
  )
}

/// [`options`] when the app said to report (`reports`) and this is not a `debug` build; `None` otherwise.
pub fn allowed_options(reports: Option<&ErrorReports>, debug: bool) -> Option<ClientOptions> {
  if debug {
    return None;
  }
  reports.map(options)
}

/// The layer that makes each ERROR line an error report while the client runs ([`start`]), and does nothing without
/// one. WARN and below are neither reports nor breadcrumbs. The line's fields go along for [`options`] to put into the
/// message, which the scrubber then cleans.
pub fn layer<S>() -> impl Layer<S>
where
  S: Subscriber + for<'span> LookupSpan<'span>,
{
  ErrorLayer::new().with_filter(filter_fn(|line| *line.level() == Level::ERROR))
}

/// Starts sending the runner's panics and ERROR log lines when the app started it with reports on and this is a
/// release build ([`allowed_options`]). Keep the guard until the runner exits: dropping it waits briefly for the
/// pending reports to leave.
pub fn start() -> Option<ClientInitGuard> {
  let options = allowed_options(ErrorReports::of_this_process().as_ref(), cfg!(debug_assertions))?;
  let environment = options.environment.clone();
  // reqwest's rustls takes the process's crypto provider; ring is the one the workspace builds.
  // Installing fails only when a provider is installed already, which then serves as well.
  let _ = rustls::crypto::ring::default_provider().install_default();
  let guard = sentry::init(options);
  tracing::info!(environment = environment.as_deref(), "error reports are sent to GlitchTip");
  Some(guard)
}
