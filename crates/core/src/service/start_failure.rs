//! A start that failed for a reason the person can act on (`docs/architecture/agent-testing.md`, "Service"; states
//! 10a and 10c of `oXVuM`): Docker was not available for a service whose per-run data is a database container, or a
//! service was not ready within its timeout. The run's conversation keeps it as a runner note with the service, the
//! cause, its last lines, the services of the start not started because of it and those the step still runs; the
//! agent reads the same facts in its start's answer. Names and lines are plain text, so a note stored once always
//! reads back.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStartFailure {
  pub service: String,
  pub cause: StartFailureCause,
  /// Its last kept lines, oldest first, as [`super::LogLine::row`] writes them, its secret values replaced; none
  /// when it never started.
  #[serde(default)]
  pub lines: Vec<String>,
  /// The services of the start after it, which were not started because it failed.
  #[serde(default)]
  pub not_started: Vec<String>,
  /// The services the step runs once it failed, its needs among them.
  #[serde(default)]
  pub running: Vec<String>,
}

/// Why a start failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cause", rename_all = "snake_case")]
pub enum StartFailureCause {
  /// No `docker` program was found in the folders of the `PATH`; `detail` names them.
  DockerMissing { detail: String },
  /// `docker` was found, but its engine did not answer `docker version`; `detail` says what it answered.
  DockerNotAnswering { detail: String },
  /// It was not ready within `secs` seconds, asked as `asked` says, and was stopped.
  NotReady { secs: u64, asked: ReadyAsked },
}

/// What Orchester asked a starting service to tell it was ready.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "check", rename_all = "snake_case")]
pub enum ReadyAsked {
  /// Whether its port `port` accepts a connection.
  Port { port: u16 },
  /// Whether `url`, its `path` on its own address, answers a 2xx or 3xx status.
  Http { url: String, path: String },
  /// Whether a line of its output matches `pattern`.
  LogLine { pattern: String },
}

impl ServiceStartFailure {
  /// This failure with `redact` applied to its lines, the only text in it a program wrote.
  pub fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    for line in &mut self.lines {
      *line = redact(line);
    }
  }
}
