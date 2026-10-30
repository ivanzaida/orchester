//! The app's panics and ERROR log lines (`app_log::error_reports`) as error reports to GlitchTip, project `desktop`,
//! in the Sentry protocol (`docs/error-reports.md`). Every report passes through `orchester_report`'s scrubber as the
//! client's `before_send` (`orchester_report::client`), and carries only the release, the environment and the tag
//! `process=app` besides what the scrubber keeps: no default PII, no traces, no sessions.
//!
//! The client starts once the application database is open: a panic before that is not reported. It sends only while
//! the person's setting allows it (`sends_error_reports`, on until turned off), and follows *Send error reports* in
//! Settings › General at once ([`turn`]): off, no report is made and the ones waiting to leave are dropped
//! (`orchester_report::switch`); on, it sends again without a restart. The app's own program reports only as the
//! installed app ([`Environment::Production`]) or a release build a QA harness started; `tools/qa-app.ps1`'s QA copy
//! (`examples/qa_app`) starts it itself as [`Environment::Qa`], though it is a debug build. Other debug builds and the
//! tests never start it. While it sends, the runners the app starts report too ([`for_runners`]), and they follow the
//! setting with it.

use std::{
  ffi::OsStr,
  sync::{Arc, OnceLock},
};

use orchester_ipc::ErrorReports;
pub use orchester_report::client::DSN;
use orchester_workspace::{runner::Runners, workspace::Workspace};
use sentry::{Client, ClientOptions, Hub, TransportFactory, transports::DefaultTransportFactory};

use crate::mcp::{Build, DISCOVERY_VARIABLE};

/// The app's version, from its `Cargo.toml`, as the release its reports name, and its runners' reports too.
pub const RELEASE: &str = concat!("orchester@", env!("CARGO_PKG_VERSION"));

/// The page that says what a report carries and what never leaves (*What a report carries* in Settings › General):
/// `docs/error-reports.md` as GitHub shows it, since a person who has only the app has no checkout.
pub const WHAT_A_REPORT_CARRIES: &str = "https://github.com/ivanzaida/orchester/blob/master/docs/error-reports.md";

/// Which of Orchester's programs a report comes from (the `process` tag the scrubber keeps).
const PROCESS: &str = "app";

/// The transport of the app's client, on while the setting is (`orchester_report::switch`).
pub type Switch = orchester_report::switch::Switch<()>;

/// The client [`start`] bound for the app, which [`turn`] switches; unset in debug builds and the tests.
static RUNNING: OnceLock<Running> = OnceLock::new();

struct Running {
  environment: Environment,
  switch: Switch,
}

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
/// [`RELEASE`], the tag `process=app`, the loaded modules' debug IDs, and the scrubber as `before_send`.
pub fn options(environment: Environment) -> ClientOptions {
  orchester_report::client::options(PROCESS, RELEASE, environment.name())
}

/// Whether the setting in the session's `workspace` allows reports; `None` without a session, and off when it cannot
/// be read (logged).
pub fn allowed(workspace: Option<&Workspace>) -> Option<bool> {
  let Some(workspace) = workspace else {
    tracing::info!("no session, so no setting to read; error reports are not sent");
    return None;
  };
  match workspace.sends_error_reports() {
    Ok(allowed) => Some(allowed),
    Err(error) => {
      tracing::warn!(%error, "cannot read whether to send error reports; not sending them");
      Some(false)
    }
  }
}

/// Starts the app's client in `environment` for every thread, sending while [`allowed`] says so; `None` without a
/// session. Keep it until the app exits: dropping it waits briefly (`orchester_report::client::FLUSH_TIMEOUT`) for
/// the pending reports to leave.
pub fn start(environment: Environment, workspace: Option<&Workspace>) -> Option<Reports> {
  let on = allowed(workspace)?;
  // reqwest's rustls takes the process's crypto provider; ring is the one the workspace builds.
  // Installing fails only when a provider is installed already, which then serves as well.
  let _ = rustls::crypto::ring::default_provider().install_default();
  let reports = Reports::new(environment, on, Arc::new(DefaultTransportFactory));
  Hub::main().bind_client(Some(reports.client.clone()));
  let running = Running {
    environment,
    switch: reports.switch.clone(),
  };
  if RUNNING.set(running).is_err() {
    tracing::error!("the app's error reports started twice; the setting turns the first client");
  }
  if on {
    tracing::info!(environment = environment.name(), "error reports are sent to GlitchTip");
  } else {
    tracing::info!("error reports are turned off");
  }
  Some(reports)
}

/// Turns the app's error reports on or off at once, as the person just chose in Settings › General: off, the app's
/// client makes no report from now on and drops the ones waiting to leave; on, it sends again. Every running runner is
/// told the same (`Runners::set_error_reports`), and every runner started later starts so. A build that never reports
/// ([`start`] did not run) has nothing to turn: the choice is only kept.
pub fn turn(on: bool, runners: &Runners) {
  let Some(running) = RUNNING.get() else {
    tracing::info!(on, "this build sends no error reports; the setting is only kept");
    return;
  };
  running.switch.set(on.then_some(()));
  runners.set_error_reports(on.then(|| for_runners(running.environment)).as_ref());
  tracing::info!(on, "error reports turned by the person");
}

/// Whether the person turned the app's running client off, so nothing it would report should even be read (a frozen
/// window's stack). `false` while no client of the app runs: a test's own client decides then.
pub fn turned_off() -> bool {
  RUNNING.get().is_some_and(|running| !running.switch.is_on())
}

/// What the runners the app starts once its client sends ([`start`]) are told (`Runners::set_error_reports`): the
/// same environment and [`RELEASE`], since a runner ships with the app.
pub fn for_runners(environment: Environment) -> ErrorReports {
  ErrorReports::new(environment.name(), RELEASE).expect("an environment's name and the release are never empty")
}

/// The app's one client and the switch the setting turns. Dropping it waits briefly for the pending reports to leave.
pub struct Reports {
  client: Arc<Client>,
  switch: Switch,
  environment: Environment,
}

impl Reports {
  /// A client bound to no hub with [`options`] for `environment`, whose reports leave through a transport of
  /// `transports`, a new one each time they are turned on; on with `on`. While off, its `before_send` makes no report.
  pub fn new(environment: Environment, on: bool, transports: Arc<dyn TransportFactory>) -> Self {
    let mut options = options(environment);
    let switch = Switch::new(transports, options.clone());
    switch.set(on.then_some(()));
    let scrub = options
      .before_send
      .take()
      .expect("orchester_report's options scrub every report");
    let gate = switch.clone();
    options.before_send = Some(Arc::new(move |event| if gate.is_on() { scrub(event) } else { None }));
    options.transport = Some(Arc::new(switch.clone()));
    Self {
      client: Arc::new(Client::from(options)),
      switch,
      environment,
    }
  }

  pub fn client(&self) -> &Arc<Client> {
    &self.client
  }

  pub fn switch(&self) -> Switch {
    self.switch.clone()
  }

  /// What the runners are told while the reports are on ([`for_runners`]); `None` while they are off.
  pub fn for_runners(&self) -> Option<ErrorReports> {
    self.switch.is_on().then(|| for_runners(self.environment))
  }
}

impl Drop for Reports {
  fn drop(&mut self) {
    self.client.close(None);
  }
}
