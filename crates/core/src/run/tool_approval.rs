//! A single tool call waiting for a person's decision while its agent waits for it. The record
//! holds what the person decides about: the command or tool, its target, and an MCP call's
//! arguments with secret values removed; never credentials.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  id::{InvocationId, RunId, StepExecutionId},
  time::Timestamp,
};

/// The most of an MCP call's arguments an approval records, in bytes. Arguments are recorded whole,
/// so the person reads everything they approve; only a pathological call is cut, and visibly.
pub const ARGUMENTS_CEILING: usize = 256 * 1024;
/// Ends arguments cut at [`ARGUMENTS_CEILING`], on a line of its own after the part kept. Built from
/// the ceiling, so what is stored and what [`cut_arguments`] finds always name the same size.
fn cut_mark() -> String {
  format!("\n… cut at {} KB", ARGUMENTS_CEILING / 1024)
}

/// `arguments` as an approval records them: whole when they fit in [`ARGUMENTS_CEILING`] bytes,
/// otherwise cut at the last character that fits and marked. Secret values must already be
/// removed: a cut must never split one.
pub fn recorded_arguments(mut arguments: String) -> String {
  if arguments.len() <= ARGUMENTS_CEILING {
    return arguments;
  }
  let end = (0..=ARGUMENTS_CEILING)
    .rev()
    .find(|end| arguments.is_char_boundary(*end))
    .expect("offset 0 is a character boundary");
  arguments.truncate(end);
  arguments.push_str(&cut_mark());
  arguments
}

/// The part of recorded arguments kept before their cut, or `None` when they were recorded whole.
pub fn cut_arguments(recorded: &str) -> Option<&str> {
  recorded.strip_suffix(cut_mark().as_str())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingToolApproval {
  pub id: String,
  pub run: RunId,
  pub execution: StepExecutionId,
  pub invocation: InvocationId,
  pub agent: String,
  pub command: bool,
  /// For a command, the reason the agent gave; for an MCP call, its arguments as
  /// [`recorded_arguments`] keeps them.
  pub scope: Option<String>,
  pub server: String,
  pub action: String,
  pub target: Option<String>,
  pub requested_at: Timestamp,
  /// What a native agent's command asks with besides the command; `None` for a CLI's request.
  pub native: Option<NativeCommandAsk>,
}

/// How a native agent's `run_command` would run (`docs/architecture/native-agent.md` §4.6, 04.H9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeCommandAsk {
  /// The shell the command string goes to; `None` when the program runs without one.
  pub shell: Option<CommandShell>,
  /// Seconds the command may run before it is stopped.
  pub time_limit_seconds: u32,
}

/// The shell a native agent's `shell: true` command runs in (§4.6, decision 3): `pwsh` where it is
/// found on Windows, otherwise Windows PowerShell 5.1; zsh on macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandShell {
  Pwsh,
  #[serde(rename = "windows_powershell")]
  WindowsPowerShell,
  Zsh,
}

impl CommandShell {
  pub const ALL: [Self; 3] = [Self::Pwsh, Self::WindowsPowerShell, Self::Zsh];

  /// How the shell is stored.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Pwsh => "pwsh",
      Self::WindowsPowerShell => "windows_powershell",
      Self::Zsh => "zsh",
    }
  }

  /// The shell's own name, which is not translated.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Pwsh => "pwsh",
      Self::WindowsPowerShell => "Windows PowerShell 5.1",
      Self::Zsh => "zsh",
    }
  }

  /// What the command string is handed to: the program where the name does not say it, and the
  /// options before the string.
  pub const fn invocation(self) -> &'static str {
    match self {
      Self::Pwsh => "-NoProfile -NonInteractive -Command",
      Self::WindowsPowerShell => "powershell.exe -NoProfile -NonInteractive -Command",
      Self::Zsh => "/bin/zsh -c",
    }
  }
}

/// A stored shell that names none of [`CommandShell::ALL`]; the database was edited or damaged.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not a known shell")]
pub struct UnknownShell(String);

impl FromStr for CommandShell {
  type Err = UnknownShell;

  fn from_str(value: &str) -> Result<Self, UnknownShell> {
    Self::ALL
      .into_iter()
      .find(|shell| shell.as_str() == value)
      .ok_or_else(|| UnknownShell(value.to_owned()))
  }
}

/// A request that is no longer pending: decided at `decided_at`, or interrupted (`decision` is
/// `None`) when the agent ended before the decision was carried out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecidedToolApproval {
  pub request: PendingToolApproval,
  pub decision: Option<ToolApprovalDecision>,
  /// Who answered; `None` for an interrupted request and for one decided before answers were recorded
  /// with who gave them.
  pub decided_by: Option<super::DecidedBy>,
  pub decided_at: Timestamp,
  /// What auto mode did with the request before it was answered.
  pub review: Option<super::review::Review>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolApprovalDecision {
  Approved,
  Denied,
}

impl ToolApprovalDecision {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Approved => "approved",
      Self::Denied => "denied",
    }
  }
}
