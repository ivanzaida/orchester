//! What `service_status` answers about a service the step started (`docs/architecture/agent-testing.md`, "Agent
//! tools"): whether it is starting, ready, crashed with its exit code, or stopped; its port and URL; and for how long
//! it has run while it runs.

use std::time::Duration;

use serde_json::json;

/// How far Orchester took a service it started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServicePhase {
  /// Its start waits until it is ready.
  Starting,
  /// Its start answered it ready, and Orchester holds it.
  Ready,
  /// Orchester stopped it: `stop_service`, a start that failed, the turn's wait for a person.
  Stopped,
}

/// Where a started service is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState {
  /// Started, and not ready yet.
  Starting,
  Ready,
  /// Its program exited on its own; `code` is `None` when a signal ended it (macOS).
  Crashed {
    code: Option<i32>,
  },
  /// Orchester stopped it while it ran.
  Stopped,
}

impl ServiceState {
  /// Where a service is that Orchester took to `phase`; `exited` is the exit code of its program once it was seen to
  /// exit on its own, which a later stop does not undo.
  pub const fn of(phase: ServicePhase, exited: Option<Option<i32>>) -> Self {
    match (phase, exited) {
      (_, Some(code)) => Self::Crashed { code },
      (ServicePhase::Starting, None) => Self::Starting,
      (ServicePhase::Ready, None) => Self::Ready,
      (ServicePhase::Stopped, None) => Self::Stopped,
    }
  }

  pub const fn name(self) -> &'static str {
    match self {
      Self::Starting => "starting",
      Self::Ready => "ready",
      Self::Crashed { .. } => "crashed",
      Self::Stopped => "stopped",
    }
  }

  /// Whether its program may still run.
  pub const fn runs(self) -> bool {
    matches!(self, Self::Starting | Self::Ready)
  }
}

/// A started service's status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceStatus {
  pub state: ServiceState,
  pub port: u16,
  pub url: String,
  /// How long it has run since its program started.
  pub uptime: Duration,
}

impl ServiceStatus {
  /// The answer: `{state, exit_code, port, url, uptime_secs}`; `exit_code` only for one that crashed, `uptime_secs`
  /// only for one that still runs.
  pub fn answer(&self) -> String {
    let mut answer = json!({"state": self.state.name(), "port": self.port, "url": self.url});
    if let ServiceState::Crashed { code } = self.state {
      answer["exit_code"] = json!(code);
    }
    if self.state.runs() {
      answer["uptime_secs"] = json!(self.uptime.as_secs());
    }
    answer.to_string()
  }
}
