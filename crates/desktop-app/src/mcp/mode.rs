//! How the app's MCP server is reached, decided once at startup (`docs/design/app-mcp-connection.md`, "Modes and
//! builds"; `docs/stable-build.md`). Only the installed app — a build `tools/install.ps1` or `tools/package-macos.sh`
//! made, which set [`INSTALLED_BUILD_VARIABLE`] while compiling — listens on the fixed address set in Settings with
//! the token kept in the secret store. Every other build (debug, `cargo run --release`, an example) takes a free port
//! and a new token on each start and writes both to a discovery file: in the folder [`DISCOVERY_VARIABLE`] names (QA
//! copies, test harnesses), else in the user's folder. So no build but the installed app takes its port or leaves a
//! credential in the person's store. The installed app refuses [`DISCOVERY_VARIABLE`] and runs without a server: a
//! program that starts it with the variable would otherwise get a working token for the person's data in a file it
//! can read.

use std::{ffi::OsString, path::PathBuf};

use orchester_workspace::{secrets::Vault, workspace::Workspace};

/// Names the folder a QA copy writes its discovery file to; `tools/qa-app.ps1` sets it to the worktree's
/// `.tmp/qa/<task>/mcp/`.
pub const DISCOVERY_VARIABLE: &str = "ORCHESTER_APP_MCP_DISCOVERY";
/// Set while compiling the installed app, by `tools/install.ps1` and `tools/package-macos.sh` only.
pub const INSTALLED_BUILD_VARIABLE: &str = "ORCHESTER_APP_INSTALLED_BUILD";

/// What kind of program this is, as it was built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
  /// The installed app.
  Installed,
  /// Any other build.
  Other,
}

impl Build {
  /// This program's kind: [`INSTALLED_BUILD_VARIABLE`] was set when it was compiled.
  pub const fn this() -> Self {
    match option_env!("ORCHESTER_APP_INSTALLED_BUILD") {
      Some(_) => Self::Installed,
      None => Self::Other,
    }
  }
}

pub enum McpMode {
  /// The installed app: the port and on/off in `workspace`'s settings, the token in `vault`.
  Installation { workspace: Workspace, vault: Vault },
  /// A QA copy or a test: its discovery file in this folder.
  Qa(PathBuf),
  /// Any other build that is not the installed app: its discovery file in the user's folder.
  Development,
}

impl McpMode {
  /// [`McpMode::choose`] for this program, its environment and the session's database.
  pub fn of_build(workspace: Option<&Workspace>) -> Option<Self> {
    Self::choose(Build::this(), std::env::var_os(DISCOVERY_VARIABLE), workspace)
  }

  /// The mode of a `build` started with [`DISCOVERY_VARIABLE`] as `discovery`. `None`, logged, when there is no
  /// server: the installed app started with the variable, or without a session (no database to read the address
  /// from; another window of this installation has it).
  pub fn choose(build: Build, discovery: Option<OsString>, workspace: Option<&Workspace>) -> Option<Self> {
    match (build, discovery) {
      (Build::Installed, Some(_)) => {
        tracing::error!(
          variable = DISCOVERY_VARIABLE,
          "the installed app does not write a discovery file; it runs without its MCP server"
        );
        None
      }
      (Build::Installed, None) => workspace.map(|workspace| Self::Installation {
        workspace: workspace.clone(),
        vault: Vault::keychain(),
      }),
      (Build::Other, Some(folder)) if !folder.is_empty() => Some(Self::Qa(PathBuf::from(folder))),
      (Build::Other, Some(_)) => {
        tracing::warn!(variable = DISCOVERY_VARIABLE, "empty; not a QA copy");
        Some(Self::Development)
      }
      (Build::Other, None) => Some(Self::Development),
    }
  }
}
