//! What belongs to the computer a run's executor runs on (`docs/architecture/remote-runners.md`, rule 1, "the
//! computer's resources"): the connections there, each with what its account and its CLI have on that computer, and
//! the home folder. The executor asks the runner's computer's resources for them and gets these values back; they are
//! serializable, so the answers can come from another process, later another computer.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
  account::AccountCli,
  connection::{AccountStartProblem, Connection, ConnectionKind},
};

/// Why a computer could not answer: what the read that failed said.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[error("{detail}")]
pub struct ComputerError {
  detail: String,
}

impl ComputerError {
  pub fn new(error: &dyn std::error::Error) -> Self {
    Self {
      detail: error.to_string(),
    }
  }
}

/// Where a computer's agent CLIs keep their configuration when their account names no folder of its own: the folder
/// the environment names (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`), else the CLI's own folder in the home folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliHomes {
  claude_code: Option<PathBuf>,
  codex: Option<PathBuf>,
  home: Option<PathBuf>,
}

impl CliHomes {
  /// The homes as `variable` reads the environment's variables, with `home` the home folder.
  pub fn new(variable: impl Fn(&str) -> Option<PathBuf>, home: Option<PathBuf>) -> Self {
    Self {
      claude_code: variable(AccountCli::ClaudeCode.folder_variable()),
      codex: variable(AccountCli::Codex.folder_variable()),
      home,
    }
  }

  pub fn home(&self) -> Option<&Path> {
    self.home.as_deref()
  }

  /// The folder `cli` keeps its configuration in without an account folder.
  fn config_folder(&self, cli: AccountCli) -> Option<PathBuf> {
    let named = match cli {
      AccountCli::ClaudeCode => &self.claude_code,
      AccountCli::Codex => &self.codex,
    };
    named
      .clone()
      .or_else(|| self.home.as_ref().map(|home| home.join(cli.own_folder_name())))
  }
}

/// A connection as the computer it was asked on has it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerConnection {
  connection: Connection,
  here: OnComputer,
}

/// What a connection has on the computer it was asked on, read as it was asked.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnComputer {
  account_problem: Option<AccountStartProblem>,
  config_folder: Option<PathBuf>,
}

impl ComputerConnection {
  /// `connection` on a computer where `folder_exists` says whether a folder is there and `homes` are its CLIs' homes.
  pub fn read(connection: Connection, folder_exists: impl FnOnce(&Path) -> bool, homes: &CliHomes) -> Self {
    let account = connection.account();
    let config_folder = match connection.kind() {
      ConnectionKind::Agent(provider) => AccountCli::of(*provider).and_then(|cli| {
        account
          .folder()
          .map(Path::to_path_buf)
          .or_else(|| homes.config_folder(cli))
      }),
      ConnectionKind::GitHub { .. } => None,
    };
    let here = OnComputer {
      account_problem: account.start_problem(folder_exists),
      config_folder,
    };
    Self { connection, here }
  }

  pub const fn connection(&self) -> &Connection {
    &self.connection
  }

  pub const fn here(&self) -> &OnComputer {
    &self.here
  }

  pub fn into_parts(self) -> (Connection, OnComputer) {
    (self.connection, self.here)
  }
}

impl OnComputer {
  /// Why no agent starts on the connection's account there: it has none, or the account's folder is not there.
  pub const fn account_problem(&self) -> Option<&AccountStartProblem> {
    self.account_problem.as_ref()
  }

  /// The folder the connection's CLI keeps its configuration in there: its account's, else the CLI's home
  /// ([`CliHomes`]); `None` for a connection that runs no agent CLI, or without a home folder.
  pub fn config_folder(&self) -> Option<&Path> {
    self.config_folder.as_deref()
  }
}
