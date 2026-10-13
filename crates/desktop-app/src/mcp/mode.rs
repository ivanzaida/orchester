//! How the app's MCP server is reached, decided once at startup (`docs/design/app-mcp-connection.md`, "Modes and
//! builds"): the installed release build listens on the fixed address set in Settings with the token kept in the
//! secret store; a copy started with [`DISCOVERY_VARIABLE`] (QA copies, test harnesses) and a development build take
//! a free port and a new token on each start and write both to a discovery file, so they never take the installed
//! app's port and agents that test them find their own copy.

use std::path::PathBuf;

use orchester_workspace::{secrets::Vault, workspace::Workspace};

/// Names the folder a QA copy writes its discovery file to; `tools/qa-app.ps1` sets it to the worktree's
/// `.tmp/qa/<task>/mcp/`. While it is set, the stable address and the stored token are not used.
pub const DISCOVERY_VARIABLE: &str = "ORCHESTER_APP_MCP_DISCOVERY";

pub enum McpMode {
  /// The installed app: the port and on/off in `workspace`'s settings, the token in `vault`.
  Installation { workspace: Workspace, vault: Vault },
  /// A QA copy or a test: its discovery file in this folder.
  Qa(PathBuf),
  /// A development build: its discovery file in the user's folder, as before the stable address.
  Development,
}

impl McpMode {
  /// QA when [`DISCOVERY_VARIABLE`] names a folder; else development in a debug build; else the installation, with
  /// the session's database and the OS keychain. `None` for a release build without a session: there is no
  /// database to read the address from, and another window of this installation has it.
  pub fn of_build(workspace: Option<&Workspace>) -> Option<Self> {
    match std::env::var_os(DISCOVERY_VARIABLE) {
      Some(folder) if !folder.is_empty() => return Some(Self::Qa(PathBuf::from(folder))),
      Some(_) => tracing::warn!(variable = DISCOVERY_VARIABLE, "empty; not a QA copy"),
      None => {}
    }
    if cfg!(debug_assertions) {
      return Some(Self::Development);
    }
    workspace.map(|workspace| Self::Installation {
      workspace: workspace.clone(),
      vault: Vault::keychain(),
    })
  }
}
