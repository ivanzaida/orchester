//! Which account an agent CLI connection runs on (`docs/design/agent-accounts.md`, "Recommended model"): a program
//! plus an account is a connection. The connection keeps the account's id and folder, so the runner, which reads
//! only the project's database, starts the CLI with the folder's variable set; the account itself (name, sign-in,
//! limits) lives in the application database.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::AgentProvider;
use crate::{
  account::{AccountCli, AgentAccount},
  connection::invocation::Platform,
  id::AgentAccountId,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConnectionAccount {
  /// The CLI's own folder: the variable stays as the environment has it. Every connection made before accounts
  /// existed is on it.
  #[default]
  Default,
  /// An account of the person's with a folder of its own.
  Account { id: AgentAccountId, folder: PathBuf },
  /// No account, after its account was removed with *No account*: its runs don't start.
  Unassigned,
}

/// Why a run can't start on a connection's account.
#[derive(Clone, Debug, PartialEq, Eq, Hash, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccountStartProblem {
  #[error("the connection has no account; choose one in its settings")]
  NoAccount,
  #[error("the account's folder {folder} is gone", folder = folder.display())]
  FolderMissing { folder: PathBuf },
  #[error("accounts other than Default are not supported for this CLI on this platform yet")]
  Unsupported,
}

/// Whether an agent that ran on `before` (`None`: nothing ran yet) and now runs on `after` moves to another
/// account: a person's decision (`docs/design/agent-accounts.md`), since another sign-in, and possibly another
/// organisation's provider, gets the project's work. Keeping the account does not, nor does a CLI's Default, the
/// CLI's own sign-in every connection ran on before accounts existed, nor no account, on which nothing runs.
pub fn moves_to_another_account(before: Option<&ConnectionAccount>, after: &ConnectionAccount) -> bool {
  match after {
    ConnectionAccount::Account { id, .. } => {
      !matches!(before, Some(ConnectionAccount::Account { id: was, .. }) if was == id)
    }
    ConnectionAccount::Default | ConnectionAccount::Unassigned => false,
  }
}

impl ConnectionAccount {
  /// The connection's choice of `account`.
  pub fn of(account: &AgentAccount) -> Self {
    match account.folder().path() {
      Some(folder) if !account.is_default() => Self::Account {
        id: account.id(),
        folder: folder.to_path_buf(),
      },
      _ => Self::Default,
    }
  }

  pub const fn is_default(&self) -> bool {
    matches!(self, Self::Default)
  }

  /// The account's id for a connection to `provider`: Default's fixed id for the CLI's own folder.
  pub fn id(&self, provider: AgentProvider) -> Option<AgentAccountId> {
    match self {
      Self::Default => AccountCli::of(provider).map(AccountCli::default_account),
      Self::Account { id, .. } => Some(*id),
      Self::Unassigned => None,
    }
  }

  /// The folder the CLI's variable is set to; `None` leaves it as the environment has it.
  pub fn folder(&self) -> Option<&Path> {
    match self {
      Self::Account { folder, .. } => Some(folder),
      Self::Default | Self::Unassigned => None,
    }
  }

  /// The variable and folder an agent of `provider` starts with, replacing the environment's.
  pub fn variable(&self, provider: AgentProvider) -> Option<(&'static str, &Path)> {
    let cli = AccountCli::of(provider)?;
    self.folder().map(|folder| (cli.folder_variable(), folder))
  }

  /// Why a run of `provider` can't start on this account on `platform`, if it can't. `folder_exists` says whether
  /// the account's folder is there.
  pub fn start_problem(
    &self,
    provider: AgentProvider,
    platform: Platform,
    folder_exists: impl FnOnce(&Path) -> bool,
  ) -> Option<AccountStartProblem> {
    match self {
      Self::Default => None,
      Self::Unassigned => Some(AccountStartProblem::NoAccount),
      Self::Account { folder, .. } => {
        if AccountCli::of(provider).is_some_and(|cli| !cli.supports_accounts_on(platform)) {
          Some(AccountStartProblem::Unsupported)
        } else if folder_exists(folder) {
          None
        } else {
          Some(AccountStartProblem::FolderMissing { folder: folder.clone() })
        }
      }
    }
  }
}
