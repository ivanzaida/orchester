//! Removing an account (decision 5): the person chooses where its connections go, it waits while a run is on it,
//! Default stays, and only a folder Orchester created may go — to the Recycle Bin / Trash, and only when asked.

use std::path::PathBuf;

use thiserror::Error;

use super::{AccountFolder, AgentAccount};
use crate::id::AgentAccountId;

/// Where the connections of a removed account go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveTo {
  /// Another account of the same CLI.
  Account(AgentAccountId),
  /// No account: their runs stop starting until the person chooses one.
  NoAccount,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RemovalError {
  #[error("Default is the CLI's own folder and can't be removed")]
  Default,
  #[error("a run is on the account; remove it after the run ends")]
  InUse,
  #[error("the connections must move to another account of the same CLI")]
  NotAnotherAccount,
  #[error("only a folder Orchester created can be moved to the Recycle Bin or Trash")]
  FolderNotCreated,
}

/// A removal the rules allow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
  account: AgentAccountId,
  move_to: MoveTo,
  trash: Option<PathBuf>,
}

impl Removal {
  /// Removing `account` of `accounts`, moving its connections to `move_to`, its folder to the Recycle Bin / Trash
  /// when `trash` says so; `in_use` while a run is on it.
  pub fn plan(
    account: &AgentAccount,
    move_to: MoveTo,
    trash: bool,
    accounts: &[AgentAccount],
    in_use: bool,
  ) -> Result<Self, RemovalError> {
    if account.is_default() {
      return Err(RemovalError::Default);
    }
    if in_use {
      return Err(RemovalError::InUse);
    }
    if let MoveTo::Account(target) = move_to {
      let fits = accounts
        .iter()
        .any(|other| other.id() == target && target != account.id() && other.cli() == account.cli());
      if !fits {
        return Err(RemovalError::NotAnotherAccount);
      }
    }
    let trash = match (trash, account.folder()) {
      (false, _) => None,
      (true, AccountFolder::Created(folder)) => Some(folder.clone()),
      (true, AccountFolder::Own | AccountFolder::Brought(_)) => return Err(RemovalError::FolderNotCreated),
    };
    Ok(Self {
      account: account.id(),
      move_to,
      trash,
    })
  }

  pub const fn account(&self) -> AgentAccountId {
    self.account
  }

  pub const fn move_to(&self) -> MoveTo {
    self.move_to
  }

  /// The folder to move to the Recycle Bin / Trash.
  pub fn trash(&self) -> Option<&std::path::Path> {
    self.trash.as_deref()
  }
}
