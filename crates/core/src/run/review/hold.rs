//! The hard rules of auto mode: what goes to the person without a review, whatever a reviewer would say.
//! Every part of a command line is checked on its own, so one harmless part never hides another: a part
//! that reads a credential, that Orchester could not read to the end (an unknown program, a script whose
//! content it does not read), that rewrites history or deletes outside the worktree, that writes in a
//! read-only step, that may reach the network while it is off, or that writes outside the worktree and the
//! step's writable folders sends the whole line to the person. A Codex escalation (a command outside its
//! sandbox) and Orchester's `run_project_tool` (a project program outside every sandbox) go to the person
//! too, as does a request too long to be reviewed whole.

use serde::{Deserialize, Serialize};

use crate::{
  orchester_tools::auto_approvable,
  permission::{AgentPermissions, NetworkAccess, RepositoryAccess, category::CommandCategory},
  run::answer::ApprovalSubject,
};

/// A hard rule that sends a request to the person without a review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewHold {
  /// It reads a credential location or store.
  Secrets,
  /// A part of it Orchester could not read to the end: an unknown program or form, or a script.
  Unreadable,
  /// It rewrites or discards history, or deletes outside the worktree.
  Destructive,
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
  /// Codex asks to run a command outside its sandbox.
  Escalation,
  /// It runs one of the project's programs, outside every sandbox.
  ProjectProgram,
}

/// The hard rule that sends `subject` to the person without a review, under the step's recorded
/// `permissions`; `None` when the reviewer may judge it.
pub fn review_hold(subject: &ApprovalSubject, permissions: Option<&AgentPermissions>) -> Option<ReviewHold> {
  let ApprovalSubject::Command(command) = subject else {
    return tool_hold(subject, permissions);
  };
  let parts = command.parts();
  if command.reads_secrets() {
    return Some(ReviewHold::Secrets);
  }
  if parts.is_empty()
    || parts
      .iter()
      .any(|part| part.category() == CommandCategory::Unknown || part.runs_script())
  {
    return Some(ReviewHold::Unreadable);
  }
  if parts
    .iter()
    .any(|part| part.category() == CommandCategory::HistoryDestructive)
  {
    return Some(ReviewHold::Destructive);
  }
  let Some(permissions) = permissions else {
    return Some(ReviewHold::PermissionsUnknown);
  };
  if permissions.repository() == RepositoryAccess::ReadOnly
    && parts.iter().any(|part| part.category() != CommandCategory::ReadOnly)
  {
    return Some(ReviewHold::ReadOnlyStep);
  }
  if permissions.network() == NetworkAccess::Off
    && parts
      .iter()
      .any(|part| part.category() == CommandCategory::Network || part.installs())
  {
    return Some(ReviewHold::NetworkOff);
  }
  parts
    .iter()
    .any(|part| part.category() == CommandCategory::OutsideWrite || part.writes_outside())
    .then_some(ReviewHold::OutsideFolders)
}

/// The hard rule for a call of an MCP tool: Orchester's `run_project_tool` never goes to the reviewer.
fn tool_hold(subject: &ApprovalSubject, permissions: Option<&AgentPermissions>) -> Option<ReviewHold> {
  match subject {
    ApprovalSubject::McpTool { server, tool } if !auto_approvable(server, tool) => Some(ReviewHold::ProjectProgram),
    _ => permissions.is_none().then_some(ReviewHold::PermissionsUnknown),
  }
}
