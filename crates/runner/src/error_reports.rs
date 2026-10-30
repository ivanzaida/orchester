//! The runner's panics and ERROR log lines as error reports to GlitchTip (`docs/error-reports.md`), the way the app
//! sends its own: the same project, client options and scrubber (`orchester_report::client`), with the tag
//! `process=runner`.
//!
//! The runner never reads the person's setting: it reports only while the app says so, in the environment and release
//! the app names. The app tells it when it starts the runner (`orchester_ipc::error_reports`), and again whenever the
//! person turns reports off or on ([`orchester_ipc::Command::SetErrorReports`], [`Switch`]). Debug builds, the tests
//! among them, never report. Its log file and stderr stay as they are.

use std::sync::Arc;

use orchester_ipc::ErrorReports;
use sentry::{Client, ClientOptions, Hub, TransportFactory, integrations::tracing::ErrorLayer, protocol::Event};
use tracing::{Level, Subscriber};
use tracing_subscriber::{Layer, filter::filter_fn, registry::LookupSpan};

/// The transport of the runner's client, which the app turns on with the environment and release its reports name,
/// and off (`orchester_report::switch`).
pub type Switch = orchester_report::switch::Switch<ErrorReports>;

/// Which of Orchester's programs a report comes from (the `process` tag the scrubber keeps).
pub const PROCESS: &str = "runner";

/// The client's options for `reports`: GlitchTip's `desktop` project, the app's environment and release, the tag
/// `process=runner`, and the scrubber as `before_send`.
pub fn options(reports: &ErrorReports) -> ClientOptions {
  orchester_report::client::options(PROCESS, reports.release().to_owned(), reports.environment().to_owned())
}

/// The layer that makes each ERROR line an error report while the client runs ([`Reports::start`]), and does nothing
/// without one. WARN and below are neither reports nor breadcrumbs. The line's fields go along for [`options`] to put
/// into the message, which the scrubber then cleans.
pub fn layer<S>() -> impl Layer<S>
where
  S: Subscriber + for<'span> LookupSpan<'span>,
{
  ErrorLayer::new().with_filter(filter_fn(|line| *line.level() == Level::ERROR))
}

/// The app turned the reports on with `reports`, or off (`orchester_ipc::Command::SetErrorReports`): `switch` follows
/// at once. A debug build has no switch, since it never reports; its log still says what the app asked.
pub fn turned(switch: Option<&Switch>, reports: Option<ErrorReports>) {
  match &reports {
    Some(reports) => tracing::info!(environment = reports.environment(), "the app turned error reports on"),
    None => tracing::info!("the app turned error reports off"),
  }
  if let Some(switch) = switch {
    switch.set(reports);
  }
}

/// The runner's one client, which reports only while its [`Switch`] is on. It stays bound for as long as the runner
/// runs, because each thread keeps the client that was bound when it first reported. Keep it until the runner exits:
/// dropping it waits briefly for the pending reports to leave.
pub struct Reports {
  client: Arc<Client>,
  switch: Switch,
}

impl Reports {
  /// The client of a release build, bound for every thread of the runner and on when the app started it with reports
  /// on; `None` in a debug build, which never reports.
  pub fn start() -> Option<Self> {
    if cfg!(debug_assertions) {
      return None;
    }
    // reqwest's rustls takes the process's crypto provider; ring is the one the workspace builds.
    // Installing fails only when a provider is installed already, which then serves as well.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let started_with = ErrorReports::of_this_process();
    if let Some(reports) = &started_with {
      tracing::info!(
        environment = reports.environment(),
        "error reports are sent to GlitchTip"
      );
    }
    let reports = Self::new(started_with, Arc::new(sentry::transports::DefaultTransportFactory));
    Hub::main().bind_client(Some(reports.client.clone()));
    Some(reports)
  }

  /// A client bound to no hub whose reports leave through a transport of `transports`, a new one each time they are
  /// turned on; on with `reports`. Its options are [`options`]', but each report names the release and the environment
  /// the switch was last turned on with.
  pub fn new(reports: Option<ErrorReports>, transports: Arc<dyn TransportFactory>) -> Self {
    let mut options = orchester_report::client::options(PROCESS, "", "");
    options.release = None;
    options.environment = None;
    let switch = Switch::new(transports, options.clone());
    switch.set(reports);
    let scrub = options
      .before_send
      .take()
      .expect("orchester_report's options scrub every report");
    let stamping = switch.clone();
    options.before_send = Some(Arc::new(move |event| scrub(stamp(&stamping, event)?)));
    options.transport = Some(Arc::new(switch.clone()));
    Self {
      client: Arc::new(Client::from(options)),
      switch,
    }
  }

  pub fn client(&self) -> &Arc<Client> {
    &self.client
  }

  pub fn switch(&self) -> Switch {
    self.switch.clone()
  }
}

impl Drop for Reports {
  fn drop(&mut self) {
    self.client.close(None);
  }
}

/// `event` with the release and the environment of the reports while `switch` is on; `None` while it is off.
fn stamp(switch: &Switch, mut event: Event<'static>) -> Option<Event<'static>> {
  let (release, environment) =
    switch.with(|reports| (reports.release().to_owned(), reports.environment().to_owned()))?;
  event.release = Some(release.into());
  event.environment = Some(environment.into());
  Some(event)
}
