//! Auto-approval (user decisions 2026-10-02 and 2026-10-03): a run, or every run of a profile, in which
//! Orchester answers the agent's requests itself. It approves only commands it knows to read or to stay
//! in the worktree, and MCP tool calls. It never approves a command that reaches the network, nor what
//! is dangerous: a write, move or delete outside the worktree and its writable folders (the temporary
//! folder included, as Run temp is not told apart), a read of a credential location, an install of
//! software or packages, anything that rewrites or discards history (whose category cannot be
//! allowed), and anything it cannot read to the end. Those still ask a person. What the step's
//! permissions refuse never reaches a person, so auto-approval never approves it either.

use super::subject::ApprovalSubject;
use crate::permission::category::{ClassifiedCommand, CommandCategory};

/// What auto-approval does with a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoVerdict {
  /// Approve it; a command's category, for its mark.
  Approve(Option<CommandCategory>),
  /// Leave it to a person, for this reason.
  Ask(AutoAsk),
}

/// Why auto-approval leaves a request to a person, most important first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoAsk {
  /// Part of the command is not read to the end.
  Unknown,
  /// It rewrites or discards history, or deletes outside the temporary folder.
  Destructive,
  /// It writes outside the worktree and its writable folders.
  OutsideWrite,
  /// It reads a credential location or store.
  Secrets,
  /// It installs software or packages.
  Installs,
  /// It reaches the network.
  Network,
}

/// Auto-approval's verdict on `subject`.
pub fn auto_verdict(subject: &ApprovalSubject) -> AutoVerdict {
  match subject {
    ApprovalSubject::McpTool { .. } => AutoVerdict::Approve(None),
    ApprovalSubject::Command(command) => command_verdict(command),
  }
}

fn command_verdict(command: &ClassifiedCommand) -> AutoVerdict {
  let ask = match command.category() {
    CommandCategory::Unknown => Some(AutoAsk::Unknown),
    CommandCategory::HistoryDestructive => Some(AutoAsk::Destructive),
    CommandCategory::OutsideWrite => Some(AutoAsk::OutsideWrite),
    CommandCategory::ReadOnly | CommandCategory::Worktree | CommandCategory::Network => None,
  }
  .or_else(|| command.reads_secrets().then_some(AutoAsk::Secrets))
  .or_else(|| command.installs().then_some(AutoAsk::Installs))
  .or_else(|| (command.category() == CommandCategory::Network).then_some(AutoAsk::Network));
  match ask {
    Some(ask) => AutoVerdict::Ask(ask),
    None => AutoVerdict::Approve(Some(command.category())),
  }
}

/// Whether Orchester auto-approves a request of a run: the run's own choice when a person made one
/// (*Approve everything in this run*, the run's switch, *Stop*), else the asking profile's setting
/// (*Approve automatically*).
pub const fn auto_approves(run: Option<bool>, profile: bool) -> bool {
  match run {
    Some(chosen) => chosen,
    None => profile,
  }
}
