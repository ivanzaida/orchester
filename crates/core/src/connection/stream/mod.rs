//! What an agent CLI reports while it works, as provider-neutral [`AgentEvent`]s. Each provider
//! has a parser for its own output format; lines a parser does not understand are kept as
//! [`AgentEvent::Raw`], so nothing the CLI prints is dropped silently (design: "Activity tab").

mod codex;

use serde::{Deserialize, Serialize};

pub use self::codex::parse_codex_line;
use crate::run::TokenUsage;

/// Which output of the CLI a line came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
  Stdout,
  Stderr,
}

/// How a command the agent ran ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
  Completed,
  Failed,
  /// The CLI's sandbox or policy refused to run it; the agent was told and continued.
  Declined,
}

/// How the agent changed one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
  Added,
  Modified,
  Deleted,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileChange {
  pub path: String,
  pub kind: FileChangeKind,
}

/// One thing an agent CLI reported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEvent {
  /// The CLI's session id, with which the session can be resumed.
  SessionStarted {
    session: String,
  },
  TurnStarted,
  /// A message the agent wrote.
  Message {
    text: String,
  },
  /// A reasoning summary, as far as the CLI emits one.
  Reasoning {
    text: String,
  },
  CommandStarted {
    item: String,
    command: String,
  },
  CommandFinished {
    item: String,
    command: String,
    output: String,
    /// `None` when the CLI reported no exit code.
    exit_code: Option<i64>,
    status: CommandStatus,
  },
  FileChanged {
    item: String,
    changes: Vec<FileChange>,
  },
  /// A turn ended with the usage it reported.
  TurnCompleted {
    usage: TokenUsage,
  },
  /// An error the CLI reported; the turn usually fails with it.
  Error {
    message: String,
  },
  TurnFailed {
    message: String,
  },
  /// A line the parser did not understand, kept as printed.
  Raw {
    stream: OutputStream,
    line: String,
  },
  /// Something went wrong around the CLI rather than in it, reported by the runner.
  RunnerProblem {
    problem: RunnerProblem,
  },
}

/// A problem the runner hit while running a step's agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "snake_case")]
pub enum RunnerProblem {
  /// The prompt uses a variable that has no value in this run; the CLI was not started.
  UnavailableVariable { variable: String },
  /// The CLI could not be started.
  CouldNotStart { detail: String },
  /// The step's changes could not be committed as a checkpoint.
  CheckpointFailed { detail: String },
}

impl AgentEvent {
  /// Stable name of the kind, as stored.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::SessionStarted { .. } => "session_started",
      Self::TurnStarted => "turn_started",
      Self::Message { .. } => "message",
      Self::Reasoning { .. } => "reasoning",
      Self::CommandStarted { .. } => "command_started",
      Self::CommandFinished { .. } => "command_finished",
      Self::FileChanged { .. } => "file_changed",
      Self::TurnCompleted { .. } => "turn_completed",
      Self::Error { .. } => "error",
      Self::TurnFailed { .. } => "turn_failed",
      Self::Raw { .. } => "raw",
      Self::RunnerProblem { .. } => "runner_problem",
    }
  }
}

/// How an agent CLI invocation ended, decided from its events and exit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum InvocationOutcome {
  /// Every turn completed and the CLI exited with 0.
  Completed,
  /// A turn failed; `message` is what the CLI said.
  TurnFailed { message: String },
  /// The CLI exited with a failure code without a failed turn.
  ExitFailed { code: Option<i32> },
  /// The CLI exited with 0 but never reported a completed turn.
  NoResult,
  /// The runner stopped the CLI.
  Terminated,
  /// The CLI was not started; a [`RunnerProblem`] says why.
  NotStarted,
}

impl InvocationOutcome {
  pub const fn is_success(&self) -> bool {
    matches!(self, Self::Completed)
  }
}

/// Follows an invocation's events: its session, the usage of its turns and whether they
/// completed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvocationTracker {
  session: Option<String>,
  usage: TokenUsage,
  completed_turns: u32,
  failure: Option<String>,
}

impl InvocationTracker {
  pub fn observe(&mut self, event: &AgentEvent) {
    match event {
      AgentEvent::SessionStarted { session } => self.session = Some(session.clone()),
      AgentEvent::TurnCompleted { usage } => {
        self.usage += *usage;
        self.completed_turns += 1;
      }
      AgentEvent::TurnFailed { message } => self.failure = Some(message.clone()),
      AgentEvent::Error { message } if self.failure.is_none() => self.failure = Some(message.clone()),
      _ => {}
    }
  }

  pub fn session(&self) -> Option<&str> {
    self.session.as_deref()
  }

  pub const fn usage(&self) -> TokenUsage {
    self.usage
  }

  /// How the invocation ended, given the CLI's exit code (`None`: no code, e.g. killed).
  pub fn outcome(&self, exit_code: Option<i32>) -> InvocationOutcome {
    if let Some(message) = &self.failure {
      return InvocationOutcome::TurnFailed {
        message: message.clone(),
      };
    }
    match exit_code {
      Some(0) if self.completed_turns > 0 => InvocationOutcome::Completed,
      Some(0) => InvocationOutcome::NoResult,
      code => InvocationOutcome::ExitFailed { code },
    }
  }
}
