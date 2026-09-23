use std::fmt;

use serde::{Deserialize, Serialize};

/// Desktop to runner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Command {
  /// Liveness check; the runner answers with [`Event::Pong`] carrying the same nonce.
  Ping { nonce: u64 },
  /// Stop the runner. It answers with [`Event::Stopped`] and exits once the desktop closes
  /// the connection.
  Shutdown { mode: ShutdownMode },
}

/// How the runner stops, matching the close confirmation in `docs/architecture/storage.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownMode {
  /// "Wait for agents to save": stop scheduling and let active steps reach a checkpoint.
  FinishAtCheckpoints,
  /// "Quit now": terminate agents without a checkpoint.
  Immediate,
}

/// Runner to desktop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Event {
  Pong {
    nonce: u64,
  },
  /// The last event of a connection: the runner has stopped all work and exits.
  Stopped {
    mode: ShutdownMode,
  },
  /// Output or exit of a supervised child process.
  Process(ProcessEvent),
}

/// Identifies a child process within one runner; never reused while the runner lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessId(u64);

impl ProcessId {
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  pub const fn get(self) -> u64 {
    self.0
  }
}

impl fmt::Display for ProcessId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "process {}", self.0)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
  Stdout,
  Stderr,
}

/// What a supervised process did. All output of a process is reported before its exit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProcessEvent {
  /// One line, without its line terminator; bytes that are not UTF-8 are replaced.
  Output {
    process: ProcessId,
    stream: OutputStream,
    line: String,
  },
  Exited {
    process: ProcessId,
    exit: ProcessExit,
  },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProcessExit {
  /// The process ended on its own.
  Exited { code: Option<i32> },
  /// The runner terminated the process and everything it started.
  Terminated,
  /// The operating system could not report how the process ended; it has been terminated.
  WaitFailed { reason: String },
}
