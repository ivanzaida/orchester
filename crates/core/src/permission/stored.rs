//! How [`AgentPermissions`] are read: the stored form of every version, a shared profile's from a team server
//! included. A version from before approval modes may have *Approve automatically (don't ask me)* on, which answered
//! what would ask a person but for what auto-approval never approved. It reads as Autopilot, which approves what it
//! approved but for what Autopilot always brings to the person, so reading it never widens.

use serde::Deserialize;

use super::{
  AgentPermissions, ApprovalMode, BrowserPermission, CommandPermission, NetworkAccess, RepositoryAccess,
  WritableFolders,
};

/// [`AgentPermissions`] as stored, with the old switch.
#[derive(Deserialize)]
pub(super) struct StoredPermissions {
  repository: RepositoryAccess,
  #[serde(default)]
  writable_folders: WritableFolders,
  commands: CommandPermission,
  network: NetworkAccess,
  #[serde(default)]
  interactive_tool_approvals: bool,
  /// *Approve automatically (don't ask me)*, left out while off; no version is saved with it any more.
  #[serde(default)]
  auto_approve: bool,
  #[serde(default)]
  browser: BrowserPermission,
  #[serde(default)]
  approval_mode: ApprovalMode,
}

impl From<StoredPermissions> for AgentPermissions {
  fn from(stored: StoredPermissions) -> Self {
    Self {
      repository: stored.repository,
      writable_folders: stored.writable_folders,
      commands: stored.commands,
      network: stored.network,
      interactive_tool_approvals: stored.interactive_tool_approvals,
      browser: stored.browser,
      approval_mode: if stored.auto_approve {
        ApprovalMode::Autopilot
      } else {
        stored.approval_mode
      },
    }
  }
}
