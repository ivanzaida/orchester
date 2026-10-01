//! The permissions an agent invocation launched with, as its CLI enforced them, recorded once with
//! the invocation before the CLI starts ([`crate::run::Invocation::permissions`]).

use serde::{Deserialize, Serialize};

use super::AgentPermissions;

/// How a person is asked about a command the permissions do not run by themselves, deciding that one
/// command in "Needs you". Launches recorded before agents asked (their CLIs denied such a command)
/// have none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutsideCommands {
  /// Claude Code's permission prompt: a command outside the allowed list, or every command.
  PermissionPrompt,
  /// Codex App Server's sandbox approval: a command that needs more than the sandbox.
  SandboxApproval,
}

/// Launch permissions: the enforced [`AgentPermissions`] and what happened to other commands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchPermissions {
  permissions: AgentPermissions,
  /// `None` for an invocation recorded before Orchester recorded it.
  outside_commands: Option<OutsideCommands>,
}

impl LaunchPermissions {
  pub const fn new(permissions: AgentPermissions, outside_commands: OutsideCommands) -> Self {
    Self {
      permissions,
      outside_commands: Some(outside_commands),
    }
  }

  /// A launch recorded before what happened to other commands was: that stays unknown.
  pub const fn recorded_before(permissions: AgentPermissions) -> Self {
    Self {
      permissions,
      outside_commands: None,
    }
  }

  pub const fn permissions(&self) -> &AgentPermissions {
    &self.permissions
  }

  pub const fn outside_commands(&self) -> Option<OutsideCommands> {
    self.outside_commands
  }
}
