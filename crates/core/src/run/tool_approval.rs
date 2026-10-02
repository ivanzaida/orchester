//! A single tool call waiting for a person's decision while its agent waits for it. The record
//! contains only display metadata; it never stores tool arguments or credentials.

use std::str::FromStr;

use thiserror::Error;

use crate::{
  id::{InvocationId, RunId, StepExecutionId},
  time::Timestamp,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingToolApproval {
  pub id: String,
  pub run: RunId,
  pub execution: StepExecutionId,
  pub invocation: InvocationId,
  pub agent: String,
  pub command: bool,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandShell {
  Pwsh,
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
  pub decided_at: Timestamp,
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
