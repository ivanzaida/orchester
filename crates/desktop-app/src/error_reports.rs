//! The app's panics as error reports to GlitchTip, project `desktop`, in the Sentry protocol (`docs/error-reports.md`).
//! Every report passes through [`ReportScrubber`] as the client's `before_send`, and carries only the release and the
//! environment besides what the scrubber keeps: no default PII, no traces, no sessions.
//!
//! The client starts once the application database is open, and only while the person's setting allows it
//! (`sends_error_reports`, on until turned off): a panic before that is not reported. The app's own program reports
//! only as the installed app ([`Environment::Production`]) or a release build a QA harness started;
//! `tools/qa-app.ps1`'s QA copy (`examples/qa_app`) starts it itself as [`Environment::Qa`], though it is a debug
//! build. Other debug builds and the tests never start it.

use std::{ffi::OsStr, time::Duration};

use orchester_report::ReportScrubber;
use orchester_workspace::workspace::Workspace;
use sentry::{
  ClientInitGuard, ClientOptions,
  integrations::{backtrace::ProcessStacktraceIntegration, contexts::ContextIntegration, panic::PanicIntegration},
};

use crate::mcp::{Build, DISCOVERY_VARIABLE};

/// The `desktop` project's client key in GlitchTip: public, it only lets a program send reports there.
pub const DSN: &str = "https://13e0bcbf57a14c2597a67d9a57d53bfe@errors.orchester.cloud/1";

/// The app's version, from its `Cargo.toml`, as the release its reports name.
pub const RELEASE: &str = concat!("orchester@", env!("CARGO_PKG_VERSION"));

/// How long the app waits for its pending reports to leave when it exits, or after a panic.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

/// Which of Orchester's programs a report comes from (the `process` tag the scrubber keeps).
const PROCESS: &str = "app";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
  /// The installed app.
  Production,
  /// A QA copy.
  Qa,
}

impl Environment {
  /// The environment of the app's own program, as it was built and started: `production` for the installed app, `qa`
  /// for a release build started with [`DISCOVERY_VARIABLE`] (a QA harness, as `McpMode` tells it). `None` for every
  /// other build, every debug build among them, which never reports.
  pub fn of_app(build: Build, debug: bool, discovery: Option<&OsStr>) -> Option<Self> {
    match (build, debug, discovery) {
      (Build::Installed, ..) => Some(Self::Production),
      (Build::Other, false, Some(folder)) if !folder.is_empty() => Some(Self::Qa),
      (Build::Other, ..) => None,
    }
  }

  /// [`Self::of_app`] for this program and its environment.
  pub fn of_this_app() -> Option<Self> {
    Self::of_app(
      Build::this(),
      cfg!(debug_assertions),
      std::env::var_os(DISCOVERY_VARIABLE).as_deref(),
    )
  }

  pub const fn name(self) -> &'static str {
    match self {
      Self::Production => "production",
      Self::Qa => "qa",
    }
  }
}

/// The client's options for `environment`: GlitchTip's `desktop` project, [`RELEASE`], and the scrubber as
/// `before_send`. Sentry's defaults keep PII, traces and sessions off. The integrations are named here rather than
/// left to `sentry::init`, so a client made from these options alone (a test's) reports panics the same way: the
/// panic hook, the OS and CPU contexts, and the stack trace's frames marked as the app's or not.
pub fn options(environment: Environment) -> ClientOptions {
  let scrubber = ReportScrubber::for_this_build();
  ClientOptions::new()
    .dsn(DSN)
    .release(RELEASE)
    .environment(environment.name())
    .send_default_pii(false)
    .shutdown_timeout(FLUSH_TIMEOUT)
    .default_integrations(false)
    .add_integration(ContextIntegration::default())
    .add_integration(PanicIntegration::default())
    .add_integration(ProcessStacktraceIntegration)
    .before_send(move |event| Some(scrubber.scrub(event)))
}

/// [`options`] when the setting in the session's `workspace` allows reports; `None` without a session, with the
/// setting off, or when it cannot be read (logged).
pub fn allowed_options(environment: Environment, workspace: Option<&Workspace>) -> Option<ClientOptions> {
  let Some(workspace) = workspace else {
    tracing::info!("no session, so no setting to read; error reports are not sent");
    return None;
  };
  match workspace.sends_error_reports() {
    Ok(true) => Some(options(environment)),
    Ok(false) => {
      tracing::info!("error reports are turned off");
      None
    }
    Err(error) => {
      tracing::warn!(%error, "cannot read whether to send error reports; not sending them");
      None
    }
  }
}

/// Starts sending the app's panics in `environment` when [`allowed_options`] allows it. Keep the guard until the app
/// exits: dropping it waits briefly ([`FLUSH_TIMEOUT`]) for the pending reports to leave.
pub fn start(environment: Environment, workspace: Option<&Workspace>) -> Option<ClientInitGuard> {
  let options = allowed_options(environment, workspace)?;
  // reqwest's rustls takes the process's crypto provider; ring is the one the workspace builds.
  // Installing fails only when a provider is installed already, which then serves as well.
  let _ = rustls::crypto::ring::default_provider().install_default();
  let guard = sentry::init(options);
  sentry::configure_scope(|scope| scope.set_tag("process", PROCESS));
  tracing::info!(environment = environment.name(), "error reports are sent to GlitchTip");
  Some(guard)
}
