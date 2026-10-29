//! Whether a runner sends error reports (`docs/error-reports.md`): the app tells the runner it starts through
//! [`ENVIRONMENT_VARIABLE`] and [`RELEASE_VARIABLE`], since the runner never reads the person's setting in
//! `orchester.db`. A runner started without them sends nothing, and keeps what it was started with until it exits.

use std::ffi::OsStr;

use tokio::process::Command;

/// The environment the runner's reports name (`production`, `qa`); set only while the app sends reports.
pub const ENVIRONMENT_VARIABLE: &str = "ORCHESTER_ERROR_REPORTS";
/// The release the runner's reports name: the app's, which the runner ships with.
pub const RELEASE_VARIABLE: &str = "ORCHESTER_ERROR_REPORTS_RELEASE";

/// The environment and the release of the reports a runner sends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorReports {
  environment: String,
  release: String,
}

impl ErrorReports {
  /// Reports in `environment` for `release`; `None` when either is empty.
  pub fn new(environment: &str, release: &str) -> Option<Self> {
    (!environment.is_empty() && !release.is_empty()).then(|| Self {
      environment: environment.to_owned(),
      release: release.to_owned(),
    })
  }

  /// What the variables say: `None` unless both are set to text.
  pub fn from_variables(environment: Option<&OsStr>, release: Option<&OsStr>) -> Option<Self> {
    Self::new(environment?.to_str()?, release?.to_str()?)
  }

  /// What this process was started with ([`Self::from_variables`]).
  pub fn of_this_process() -> Option<Self> {
    Self::from_variables(
      std::env::var_os(ENVIRONMENT_VARIABLE).as_deref(),
      std::env::var_os(RELEASE_VARIABLE).as_deref(),
    )
  }

  pub fn environment(&self) -> &str {
    &self.environment
  }

  pub fn release(&self) -> &str {
    &self.release
  }

  /// Sets the variables of the runner `command` starts to `reports`, or removes them, so a runner never reports
  /// because the app itself was started with them.
  pub(crate) fn pass(reports: Option<&Self>, command: &mut Command) {
    match reports {
      Some(reports) => command
        .env(ENVIRONMENT_VARIABLE, &reports.environment)
        .env(RELEASE_VARIABLE, &reports.release),
      None => command.env_remove(ENVIRONMENT_VARIABLE).env_remove(RELEASE_VARIABLE),
    };
  }
}
