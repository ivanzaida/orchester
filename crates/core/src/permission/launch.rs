//! The permissions an agent invocation launched with, as its CLI enforced them, recorded once with
//! the invocation before the CLI starts ([`crate::run::Invocation::permissions`]).

use serde::{Deserialize, Serialize};

use super::AgentPermissions;

/// What happens to a command the permissions do not run by themselves: one outside the allowed list
/// (Claude Code) or one that needs more than the sandbox (Codex).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutsideCommands {
  /// The CLI denies it and tells the agent.
  Denied,
  /// The CLI asks a person, who decides that one command in "Needs you".
  Asked,
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
