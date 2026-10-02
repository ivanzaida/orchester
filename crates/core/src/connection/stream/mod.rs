//! What an agent CLI reports while it works, as provider-neutral [`AgentEvent`]s: Claude Code's
//! `stream-json` lines are read by [`ClaudeStream`], Codex App Server's JSON-RPC messages by
//! [`crate::connection::app_server`]. Lines a parser does not understand are kept as
//! [`AgentEvent::Raw`], so nothing the CLI prints is dropped silently (design: "Activity tab").

mod claude;
mod redaction;

use serde::{Deserialize, Serialize};

pub use self::claude::ClaudeStream;
use crate::{code_index::RipgrepFallback, connection::mcp::McpServerProblem, run::TokenUsage};

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
  /// Live account limits, sent to the desktop without persisting the reading.
  PlanLimits {
    limits: super::plan_limits::PlanLimits,
  },
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
  /// The CLI's permissions denied a tool call (`command`, as [`AgentEvent::CommandStarted`] names
  /// it); the agent was told and went on without it.
  PermissionDenied {
    item: String,
    command: String,
  },
  /// The CLI is not signed in, so it could do nothing; `message` is what it said.
  NotSignedIn {
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
  /// The agent works without MCP server `server`: the runner left it out before the CLI started, or the
  /// CLI reported that it did not start. `None` when the CLI did not say which server.
  McpServer {
    server: Option<String>,
    problem: McpServerProblem,
  },
  /// The agent called `tool` of MCP server `server` (as tool call `item`), and nobody was asked: the step asks
  /// before MCP actions, and the server lists the tool among those that run without asking.
  McpToolNotAsked {
    item: String,
    server: String,
    tool: String,
  },
  /// `tool` of MCP server `server` is listed to run without asking, but Claude Code would name it as
  /// it could name a tool of `other`, so it got no allow rule and asks like any other tool.
  McpExemptionNotApplied {
    server: String,
    tool: String,
    other: String,
  },
  /// Codex App Server's MCP call `item` of `tool` of server `server` ended as `status`; `asked` when
  /// Codex asked for approval of it first.
  McpToolFinished {
    item: String,
    server: String,
    tool: String,
    status: CommandStatus,
    asked: bool,
  },
  /// Codex asked for approval of the call `item` of `tool` of MCP server `server` in a step that does
  /// not ask before MCP actions; Orchester declined it at once, without asking anyone.
  McpToolDeclined {
    item: String,
    server: String,
    tool: String,
  },
  /// A tool step started the program of command tool `tool`; `command` is its command line as
  /// [`crate::resource::CommandAction::command_line`] writes it.
  ToolCommandStarted {
    tool: String,
    command: String,
  },
  /// The program ended as `result` says, with `exit_code` when it exited on its own. `output` is
  /// the end of what it printed (both streams, in order) as far as the tool keeps it; the
  /// `omitted_bytes` before it were not kept.
  ToolCommandFinished {
    tool: String,
    command: String,
    result: ToolCommandResult,
    exit_code: Option<i64>,
    output: String,
    omitted_bytes: u64,
  },
}

/// How a command tool's program ended, for its step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCommandResult {
  /// It exited with a code the tool expects: the step passes.
  Passed,
  /// It exited with another code, or could not report one: the step fails.
  Failed,
  /// It did not finish within the tool's timeout and was ended: the step fails.
  TimedOut,
  /// The run ended it (cancel, a limit, another step's failure); the run decides what follows.
  Stopped,
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
  /// The step's changes conflict with what other steps changed on the run's branch in `files`;
  /// they stay on the step's own branch.
  MergeConflict { branch: String, files: Vec<String> },
  /// The execution used its step budget of `budget` tokens; its agent was ended.
  StepBudgetReached { budget: u64 },
  /// A command tool's `program` is no file on the `PATH` or in the worktree; it was not started.
  ProgramNotFound { program: String },
  /// Claude Code's permission prompt never reached the runner, so no action could be approved:
  /// every call it asked about was denied as unavailable.
  PermissionPromptUnreachable,
  /// The provider's API refused `model` to the CLI's version `installed`: it needs `required` or
  /// newer.
  CliTooOld {
    model: String,
    installed: String,
    required: String,
  },
  /// The codebase index of the run's base revision could not be built; the agent works without it.
  CodebaseIndexUnavailable { detail: String },
  /// ripgrep was found neither on the agent's `PATH` nor with an agent CLI; `fallback` says how the
  /// agent searches instead.
  RipgrepNotFound {
    #[serde(default)]
    fallback: RipgrepFallback,
  },
  /// ripgrep was found but could not be copied where the agent can run it; `fallback` says how the
  /// agent searches instead.
  RipgrepUnavailable {
    detail: String,
    #[serde(default)]
    fallback: RipgrepFallback,
  },
  /// The step's handoff was not kept as task memory.
  MemoryNotSaved { reason: MemoryNotSavedReason },
  /// `records` task or project memory records could not be read back; agents get the others.
  MemoryUnreadable { records: u64 },
  /// Memory could not be checked for unreadable records; `detail` is the storage error. The agent
  /// started all the same.
  MemoryScanFailed { detail: String },
  /// The worktree has the repository's own Codex settings in `folder` (`.codex\` in the worktree),
  /// which Codex did not load for the agent; the agent started all the same.
  CodexSettingsIgnored { folder: String },
  /// The CLI wrote `lines` more lines to standard error than the activity keeps; they were left out.
  StderrLeftOut { lines: u64 },
}

/// Why a step's handoff was not kept as task memory. It names a secret, never its value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum MemoryNotSavedReason {
  /// It holds the value of secret `secret`.
  ContainsSecret { secret: String },
  /// The keychain could not be read to check it for the value of secret `secret`.
  KeychainUnavailable { secret: String },
  /// The project database could not store it; `detail` is the storage error, which never holds a
  /// record's text.
  StorageFailed { detail: String },
}

impl AgentEvent {
  /// Stable name of the kind, as stored.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::PlanLimits { .. } => "plan_limits",
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
      Self::PermissionDenied { .. } => "permission_denied",
      Self::NotSignedIn { .. } => "not_signed_in",
      Self::Raw { .. } => "raw",
      Self::RunnerProblem { .. } => "runner_problem",
      Self::McpServer { .. } => "mcp_server",
      Self::McpToolNotAsked { .. } => "mcp_tool_not_asked",
      Self::McpExemptionNotApplied { .. } => "mcp_exemption_not_applied",
      Self::McpToolDeclined { .. } => "mcp_tool_declined",
      Self::McpToolFinished { .. } => "mcp_tool_finished",
      Self::ToolCommandStarted { .. } => "tool_command_started",
      Self::ToolCommandFinished { .. } => "tool_command_finished",
    }
  }
}

/// How an agent CLI invocation ended, decided from its events and exit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum InvocationOutcome {
  /// Every turn completed and the CLI exited with 0, or a command tool's program exited with a
  /// code the tool expects.
  Completed,
  /// A turn failed; `message` is what the CLI said.
  TurnFailed { message: String },
  /// The CLI is not signed in; `message` is what it said.
  NotSignedIn { message: String },
  /// The CLI exited with a failure code without a failed turn, or a command tool's program with a
  /// code the tool does not expect.
  ExitFailed { code: Option<i32> },
  /// The CLI exited with 0 but never reported a completed turn.
  NoResult,
  /// The runner stopped the CLI.
  Terminated,
  /// The CLI was not started; a [`RunnerProblem`] says why.
  NotStarted,
  /// A command tool's program did not finish within the tool's timeout; it was ended.
  TimedOut,
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
  failure: Option<Failure>,
  last_message: Option<String>,
}

impl InvocationTracker {
  pub fn observe(&mut self, event: &AgentEvent) {
    match event {
      AgentEvent::SessionStarted { session } => self.session = Some(session.clone()),
      AgentEvent::TurnCompleted { usage } => {
        self.usage += *usage;
        self.completed_turns += 1;
      }
      AgentEvent::NotSignedIn { message } => self.failure = Some(Failure::NotSignedIn(message.clone())),
      AgentEvent::TurnFailed { message } if !self.not_signed_in() => {
        self.failure = Some(Failure::Turn(message.clone()));
      }
      AgentEvent::Message { text } => self.last_message = Some(text.clone()),
      AgentEvent::Error { message } if self.failure.is_none() => self.failure = Some(Failure::Turn(message.clone())),
      _ => {}
    }
  }

  const fn not_signed_in(&self) -> bool {
    matches!(self.failure, Some(Failure::NotSignedIn(_)))
  }

  pub fn session(&self) -> Option<&str> {
    self.session.as_deref()
  }

  pub const fn usage(&self) -> TokenUsage {
    self.usage
  }

  /// The last message the agent wrote: its final message once the invocation ended.
  pub fn last_message(&self) -> Option<&str> {
    self.last_message.as_deref()
  }

  /// How the invocation ended, given the CLI's exit code (`None`: no code, e.g. killed).
  pub fn outcome(&self, exit_code: Option<i32>) -> InvocationOutcome {
    match &self.failure {
      Some(Failure::Turn(message)) => {
        return InvocationOutcome::TurnFailed {
          message: message.clone(),
        };
      }
      Some(Failure::NotSignedIn(message)) => {
        return InvocationOutcome::NotSignedIn {
          message: message.clone(),
        };
      }
      None => {}
    }
    match exit_code {
      Some(0) if self.completed_turns > 0 => InvocationOutcome::Completed,
      Some(0) => InvocationOutcome::NoResult,
      code => InvocationOutcome::ExitFailed { code },
    }
  }
}

/// Why an invocation failed, as its events tell.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Failure {
  Turn(String),
  NotSignedIn(String),
}
