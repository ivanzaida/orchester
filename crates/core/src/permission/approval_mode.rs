//! Who answers an agent's tool requests (`docs/design/approval-modes.md`): Autopilot approves them at once, Manual
//! runs only what the profile allows and sends the rest to the reviewer, then to a person. Decisions (questions,
//! plans, limits, merge conflicts) come to a person in both.

use serde::{Deserialize, Serialize};

/// A profile version's approval mode. Manual has no list of its own: its allowed commands are the profile's
/// ([`super::AgentPermissions::commands`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
  /// Every tool request is approved at once, but for work outside the run's worktree, its Sources and the agent's
  /// temp folders, pushes to a remote and changes to refs other than the run's branch, which come to a person.
  Autopilot,
  /// The profile's allowed commands (and what its file, network and MCP permissions allow) run at once; anything
  /// else goes to the reviewer, and what it denies or is unsure about to a person. Profiles saved before approval
  /// modes existed are Manual.
  #[default]
  Manual,
}

impl ApprovalMode {
  pub const fn is_manual(&self) -> bool {
    matches!(self, Self::Manual)
  }

  /// Whether this mode asks before more than `other` does: Manual is stricter than Autopilot, so going from Manual
  /// to Autopilot widens ([`super::widening::Widening::Autopilot`]) and the reverse never does.
  pub const fn is_stricter_than(self, other: Self) -> bool {
    matches!((self, other), (Self::Manual, Self::Autopilot))
  }
}
