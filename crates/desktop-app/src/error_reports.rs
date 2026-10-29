//! The app's panics and ERROR log lines (`app_log::error_reports`) as error reports to GlitchTip, project `desktop`,
//! in the Sentry protocol (`docs/error-reports.md`). Every report passes through `orchester_report`'s scrubber as the
//! client's `before_send` (`orchester_report::client`), and carries only the release, the environment and the tag
//! `process=app` besides what the scrubber keeps: no default PII, no traces, no sessions.
//!
//! The client starts once the application database is open, and only while the person's setting allows it
//! (`sends_error_reports`, on until turned off): a panic before that is not reported. The app's own program reports
//! only as the installed app ([`Environment::Production`]) or a release build a QA harness started;
//! `tools/qa-app.ps1`'s QA copy (`examples/qa_app`) starts it itself as [`Environment::Qa`], though it is a debug
//! build. Other debug builds and the tests never start it. While it runs, the runners the app starts report too
//! ([`for_runners`]).

use std::ffi::OsStr;

use orchester_ipc::ErrorReports;
pub use orchester_report::client::DSN;
use orchester_workspace::workspace::Workspace;
use sentry::{ClientInitGuard, ClientOptions};

use crate::mcp::{Build, DISCOVERY_VARIABLE};

/// The app's version, from its `Cargo.toml`, as the release its reports name, and its runners' reports too.
pub const RELEASE: &str = concat!("orchester@", env!("CARGO_PKG_VERSION"));

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

/// The client's options for `environment` (`orchester_report::client::options`): GlitchTip's `desktop` project,
/// [`RELEASE`], the tag `process=app`, and the scrubber as `before_send`.
pub fn options(environment: Environment) -> ClientOptions {
  orchester_report::client::options(PROCESS, RELEASE, environment.name())
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

/// Starts sending the app's panics and ERROR log lines in `environment` when [`allowed_options`] allows it. Keep the
/// guard until the app exits: dropping it waits briefly (`orchester_report::client::FLUSH_TIMEOUT`) for the pending
/// reports to leave.
pub fn start(environment: Environment, workspace: Option<&Workspace>) -> Option<ClientInitGuard> {
  let options = allowed_options(environment, workspace)?;
  // reqwest's rustls takes the process's crypto provider; ring is the one the workspace builds.
  // Installing fails only when a provider is installed already, which then serves as well.
  let _ = rustls::crypto::ring::default_provider().install_default();
  let guard = sentry::init(options);
  tracing::info!(environment = environment.name(), "error reports are sent to GlitchTip");
  Some(guard)
}

/// What the runners the app starts once its client runs ([`start`]) are told (`Runners::send_error_reports`): the
/// same environment and [`RELEASE`], since a runner ships with the app.
pub fn for_runners(environment: Environment) -> ErrorReports {
  ErrorReports::new(environment.name(), RELEASE).expect("an environment's name and the release are never empty")
}
