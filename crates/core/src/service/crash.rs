//! A service that crashed after it was ready (`docs/architecture/agent-testing.md`, "Service"; card `FwHrv`): its
//! program exited on its own while Orchester held it. The crash carries the service, its exit code, how long it ran,
//! its last lines and the services Orchester stopped because they need it. The agent reads it as a notice
//! ([`ServiceCrash::notice`]); the run's conversation keeps it as a runner note for the person, its names and lines as
//! plain text, so a note stored once always reads back.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{ServiceLog, ServiceName};

/// How many of its last lines of output a crash carries, as a failed start's error shows.
pub const CRASH_LINES: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceCrash {
  pub service: String,
  /// `None` when a signal ended it (macOS).
  pub exit_code: Option<i32>,
  /// How long it had run since its program started, in milliseconds.
  pub uptime_ms: u64,
  /// Its last kept lines, oldest first, as [`super::LogLine::row`] writes them, its secret values replaced.
  pub lines: Vec<String>,
  /// The services Orchester stopped because they need it, also through others.
  pub stopped: Vec<String>,
}

impl ServiceCrash {
  /// The crash of `service`, which exited with `exit_code` after `uptime`, its output in `log`; `stopped` were stopped
  /// because they need it.
  pub fn new(
    service: &ServiceName,
    (exit_code, uptime): (Option<i32>, Duration),
    log: &ServiceLog,
    stopped: &[ServiceName],
  ) -> Self {
    Self {
      service: service.to_string(),
      exit_code,
      uptime_ms: u64::try_from(uptime.as_millis()).unwrap_or(u64::MAX),
      lines: log.last_rows(CRASH_LINES),
      stopped: stopped.iter().map(ToString::to_string).collect(),
    }
  }

  /// What the agent is told: which service crashed, how and when, what was stopped with it, and its last lines.
  pub fn notice(&self) -> String {
    let mut notice = format!(
      "Orchester notice: the service `{}` crashed: it {}, {:.1} s after it started.",
      self.service,
      exited(self.exit_code),
      Duration::from_millis(self.uptime_ms).as_secs_f64()
    );
    if !self.stopped.is_empty() {
      let stopped: Vec<String> = self.stopped.iter().map(|name| format!("`{name}`")).collect();
      notice.push_str(&format!(
        " Orchester stopped the services that need it: {}.",
        stopped.join(", ")
      ));
    }
    if self.lines.is_empty() {
      notice.push_str(" It wrote no line.");
    } else {
      notice.push_str(" Its last lines:");
      for line in &self.lines {
        notice.push('\n');
        notice.push_str(line);
      }
    }
    notice
  }

  /// This crash with `redact` applied to its lines, the only text in it a program wrote.
  pub fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    for line in &mut self.lines {
      *line = redact(line);
    }
  }
}

/// How a program ended on its own: `exited with code 3`, or `was ended by a signal` without a code (macOS).
pub fn exited(code: Option<i32>) -> String {
  code.map_or_else(
    || "was ended by a signal".to_owned(),
    |code| format!("exited with code {code}"),
  )
}
