//! The hard rules of auto mode: what goes to the person without a review, whatever a reviewer would say.
//! Every part of a command line Orchester can read is checked on its own, so one harmless part never hides
//! another: a part that reads a credential, rewrites history or deletes outside the worktree, pushes
//! anything but the run's own branch to its `origin` ([`super::push`]), writes in a read-only step, may reach the
//! network while it is off, or writes outside the worktree and the step's writable folders sends the
//! whole line to the person.
//!
//! A part Orchester cannot read to the end (an unknown program or form, a script whose content it does
//! not read) goes to the reviewer, which is told so ([`unread_parts`]), except in a step whose network is
//! off or that only reads: Orchester cannot hold such a part to those limits, so the person decides.
//! Where its output goes is checked all the same. A Codex escalation (a command outside its sandbox)
//! always goes to the person. Orchester's
//! `run_project_tool` (a project program outside every sandbox) and a request too long to be reviewed
//! whole go to the person too.

use serde::{Deserialize, Serialize};

use super::push::{PushFacts, disguised_git, is_push, reviewable_push};
use crate::{
  orchester_tools::auto_approvable,
  permission::{
    AgentPermissions, NetworkAccess, RepositoryAccess,
    category::{CommandCategory, CommandPart},
  },
  run::answer::ApprovalSubject,
};

/// A hard rule that sends a request to the person without a review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewHold {
  /// It reads a credential location or store.
  Secrets,
  /// A part of it Orchester could not read to the end, in a step whose network is off or that only reads.
  Unreadable,
  /// It rewrites or discards history, or deletes outside the worktree.
  Destructive,
  /// It pushes something other than the run's own branch to `origin`, or forces a push.
  Push,
  /// The step only reads the repository, and the command may write.
  ReadOnlyStep,
  /// The step's network is off, and the command may reach it or installs.
  NetworkOff,
  /// It writes outside the worktree and the step's writable folders.
  OutsideFolders,
  /// The step's permissions were not recorded, so nothing tells what is within them.
  PermissionsUnknown,
  /// The request is longer than a review reads whole.
  TooLong,
  /// Codex asks to run it outside its sandbox.
  Escalation,
  /// It runs one of the project's programs, outside every sandbox.
  ProjectProgram,
}

/// What the hard rules know besides the request: the step's recorded permissions, where a push of the
/// run would go, and whether the request is a Codex escalation.
#[derive(Clone, Copy, Debug, Default)]
pub struct HoldRules<'a> {
  pub permissions: Option<&'a AgentPermissions>,
  pub push: PushFacts<'a>,
  pub escalation: bool,
}

/// The hard rule that sends `subject` to the person without a review, under `rules`; `None` when the
/// reviewer may judge it.
pub fn review_hold(subject: &ApprovalSubject, rules: &HoldRules<'_>) -> Option<ReviewHold> {
  let ApprovalSubject::Command(command) = subject else {
    return tool_hold(subject, rules.permissions);
  };
  if command.reads_secrets() {
    return Some(ReviewHold::Secrets);
  }
  // Approved, an escalation runs the command outside Codex's sandbox, beyond any step's permissions.
  if rules.escalation {
    return Some(ReviewHold::Escalation);
  }
  if disguised_git(command) {
    return Some(ReviewHold::Push);
  }
  let Some(permissions) = rules.permissions else {
    return Some(ReviewHold::PermissionsUnknown);
  };
  let confined = permissions.network() == NetworkAccess::Off || permissions.repository() == RepositoryAccess::ReadOnly;
  let parts = command.parts();
  if parts.is_empty() || (confined && parts.iter().any(unread)) {
    return Some(ReviewHold::Unreadable);
  }
  if (0..parts.len()).any(|at| is_push(&parts[at]) && !reviewable_push(parts, at, &rules.push)) {
    return Some(ReviewHold::Push);
  }
  if parts
    .iter()
    .any(|part| part.category() == CommandCategory::HistoryDestructive && !is_push(part))
  {
    return Some(ReviewHold::Destructive);
  }
  outside_permissions(parts, permissions)
}

/// The hard rule a part outside the step's permissions breaks: a write in a read-only step, the network
/// or an install while it is off, a write outside the folders.
fn outside_permissions(parts: &[CommandPart], permissions: &AgentPermissions) -> Option<ReviewHold> {
  if permissions.repository() == RepositoryAccess::ReadOnly
    && parts.iter().any(|part| part.category() != CommandCategory::ReadOnly)
  {
    return Some(ReviewHold::ReadOnlyStep);
  }
  if permissions.network() == NetworkAccess::Off
    && parts
      .iter()
      .any(|part| part.category() == CommandCategory::Network || part.installs() || is_push(part))
  {
    return Some(ReviewHold::NetworkOff);
  }
  parts
    .iter()
    .any(|part| part.category() == CommandCategory::OutsideWrite || part.writes_outside())
    .then_some(ReviewHold::OutsideFolders)
}

/// Whether Orchester could not read `part` to the end: an unknown program or form, or a script.
fn unread(part: &CommandPart) -> bool {
  part.category() == CommandCategory::Unknown || part.runs_script()
}

/// How many parts of `subject` Orchester could not read to the end; the reviewer is told.
pub fn unread_parts(subject: &ApprovalSubject) -> usize {
  match subject {
    ApprovalSubject::Command(command) => command.parts().iter().filter(|part| unread(part)).count(),
    ApprovalSubject::McpTool { .. } => 0,
  }
}

/// The hard rule for a call of an MCP tool: Orchester's `run_project_tool` never goes to the reviewer.
fn tool_hold(subject: &ApprovalSubject, permissions: Option<&AgentPermissions>) -> Option<ReviewHold> {
  match subject {
    ApprovalSubject::McpTool { server, tool } if !auto_approvable(server, tool) => Some(ReviewHold::ProjectProgram),
    _ => permissions.is_none().then_some(ReviewHold::PermissionsUnknown),
  }
}
