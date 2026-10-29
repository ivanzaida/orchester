use std::time::Duration;

use super::ServiceError;

/// The path an HTTP readiness check asks on the service's own URL, e.g. `/healthz`; a 2xx or 3xx answer means
/// ready.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HttpPath(String);

impl HttpPath {
  pub const MAX_CHARS: usize = 2048;

  pub fn new(value: &str) -> Result<Self, ServiceError> {
    let valid = value.starts_with('/')
      && value.chars().count() <= Self::MAX_CHARS
      && !value.chars().any(|c| c.is_whitespace() || c.is_control());
    if !valid {
      return Err(ServiceError::HttpPath(value.to_owned()));
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A regular expression a line of the service's output matches once it is ready, e.g. `ready in \d+ ms`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LogPattern(String);

impl LogPattern {
  pub const MAX_CHARS: usize = 1024;

  pub fn new(value: &str) -> Result<Self, ServiceError> {
    if value.chars().count() > Self::MAX_CHARS {
      return Err(ServiceError::LogPatternTooLong);
    }
    if value.is_empty() {
      return Err(ServiceError::LogPattern {
        pattern: String::new(),
        reason: "it is empty, so every line would match".to_owned(),
      });
    }
    if let Err(error) = regex::Regex::new(value) {
      return Err(ServiceError::LogPattern {
        pattern: value.to_owned(),
        reason: error.to_string(),
      });
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// How long the start waits for the service to be ready; then it fails with the last log lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadyTimeout(u64);

impl ReadyTimeout {
  pub const MAX_SECS: u64 = 60 * 60;

  pub fn from_secs(secs: u64) -> Result<Self, ServiceError> {
    if (1..=Self::MAX_SECS).contains(&secs) {
      Ok(Self(secs))
    } else {
      Err(ServiceError::ReadyTimeout)
    }
  }

  pub const fn secs(self) -> u64 {
    self.0
  }

  pub const fn duration(self) -> Duration {
    Duration::from_secs(self.0)
  }
}

/// What tells Orchester the service is ready.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReadyCheck {
  /// The service's port accepts connections.
  PortOpen,
  Http(HttpPath),
  LogLine(LogPattern),
}

/// When a started service counts as ready, checked until the timeout.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Readiness {
  check: ReadyCheck,
  timeout: ReadyTimeout,
}

impl Readiness {
  pub const fn new(check: ReadyCheck, timeout: ReadyTimeout) -> Self {
    Self { check, timeout }
  }

  pub const fn check(&self) -> &ReadyCheck {
    &self.check
  }

  pub const fn timeout(&self) -> ReadyTimeout {
    self.timeout
  }
}
