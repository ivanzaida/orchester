//! What a native agent's tool call did, as its transcript row shows it (04.H10,
//! `docs/architecture/native-agent.md` §2.2, decision 11): the tool, what a file tool read, found or
//! changed, the shell a command ran in, and for a command that did not run who declined it and the
//! result the agent was given instead.

use serde::{Deserialize, Serialize};

use super::FileChangeKind;
use crate::{native::tools::ToolName, run::CommandShell};

/// One native tool call, told by [`super::AgentEvent::NativeCall`] right before the event that ends
/// the call: its [`super::AgentEvent::CommandFinished`], or the [`super::AgentEvent::PermissionDenied`]
/// of a command that did not run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCall {
  pub tool: ToolName,
  /// What the call read, found or changed; `None` when it failed before it did any of that.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub detail: Option<CallDetail>,
  /// The command did not run: who declined it, and what the agent was told.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub declined: Option<Declined>,
}

/// What one call did, by its tool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CallDetail {
  /// `read_file`: lines `first`–`last` of a file of `bytes` bytes (a binary file reads no lines: both 0).
  Read { first: u64, last: u64, bytes: u64 },
  /// `list_files`: the entries it listed.
  Listed { entries: u64 },
  /// `search`: where it searched (its path or glob, `None` for the whole worktree), and what it found.
  Searched {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    matches: u64,
    files: u64,
  },
  /// `edit_file` or `write_file`: how the file changed, in lines.
  Changed {
    change: FileChangeKind,
    added: u64,
    removed: u64,
  },
  /// `run_command`: the shell its string went to; `None` when the program ran without one.
  Command {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shell: Option<CommandShell>,
  },
}

/// A command that did not run, and the result the agent got for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declined {
  pub by: DeclinedBy,
  pub result: String,
}

/// Who kept a command from running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclinedBy {
  /// A person denied it; the agent continues without it (decision 2).
  Person,
  /// Orchester refused it: it could not be read, or its working folder lies outside the worktree.
  Orchester,
}

impl NativeCall {
  pub(super) fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    if let Some(CallDetail::Searched { scope: Some(scope), .. }) = &mut self.detail {
      *scope = redact(scope);
    }
    if let Some(declined) = &mut self.declined {
      declined.result = redact(&declined.result);
    }
  }
}
