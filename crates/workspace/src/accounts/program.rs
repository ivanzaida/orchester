//! The program of each agent CLI on this computer, as Settings › Accounts says it (`Agents on accounts` A1 `aOR4w`,
//! A3 `t2YD7`): the one the person set for the CLI, else the newest copy on the search path, with the version it
//! reports. Every project's connection on an account of the CLI is made on it. Asking a program for its version
//! runs it (`--version`, read-only) and blocks; the UI calls this off the UI thread.

use std::path::PathBuf;

use orchester_core::account::AccountCli;
use orchester_store::StoreError;

use crate::{connections::probe, workspace::Workspace};

/// The program of an agent CLI on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliProgram {
  pub cli: AccountCli,
  /// Where the program is: the path the person set, else the newest copy found; `None` when none is set and none
  /// is found.
  pub path: Option<PathBuf>,
  /// The version the program reported; `None` when it is missing or did not answer as the CLI.
  pub version: Option<String>,
  /// Whether the person set the path, so the search path is not looked at.
  pub set: bool,
}

impl CliProgram {
  /// Whether accounts of the CLI can run: a program is there and answered as the CLI.
  pub const fn runs(&self) -> bool {
    self.version.is_some()
  }
}

impl Workspace {
  /// The program of `cli` on this computer now.
  pub fn cli_program(&self, cli: AccountCli) -> Result<CliProgram, StoreError> {
    let provider = cli.provider();
    let installed = match self.agent_program(cli)? {
      Some(path) => Some((probe::installed_at(provider, path), true)),
      None => probe::newest_agent(provider).map(|installed| (installed, false)),
    };
    Ok(match installed {
      Some((installed, set)) => CliProgram {
        cli,
        path: Some(installed.executable),
        version: installed.version,
        set,
      },
      None => CliProgram {
        cli,
        path: None,
        version: None,
        set: false,
      },
    })
  }
}
