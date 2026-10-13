use std::{io, path::PathBuf};

use orchester_core::{
  account::{AccountError, FolderProblem, RemovalError},
  connection::{ConnectionError, invocation::InvocationError},
  name::Name,
};
use orchester_store::{RunStoreError, StoreError};
use thiserror::Error;

use crate::messages::{self, Message};

#[derive(Debug, Error)]
pub enum AccountsError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Runs(#[from] RunStoreError),
  #[error(transparent)]
  Account(AccountError),
  #[error("the folder can't be an account's: {0:?}")]
  Folder(FolderProblem),
  #[error("the account's folder could not be created: {0}")]
  CreateFolder(io::Error),
  /// Orchester has no application data folder for accounts on this platform.
  #[error("no folder for new accounts on this platform")]
  NoFolderHere,
  #[error(transparent)]
  Removal(RemovalError),
  /// Listed projects that could not be read: a run there may be on the account.
  #[error("projects could not be read: {}", .0.iter().map(Name::as_str).collect::<Vec<_>>().join(", "))]
  ProjectsUnreadable(Vec<Name>),
  #[error("the folder could not be moved to the Recycle Bin or Trash: {0}")]
  Trash(io::Error),
  /// Claude Code accounts other than Default on macOS (decision 10).
  #[error("accounts other than Default are not supported for this CLI on this platform yet")]
  Unsupported,
  #[error("the CLI is not on the PATH")]
  CliMissing,
  #[error("the account's folder {0} is gone")]
  FolderMissing(PathBuf),
  #[error("the sign-in terminal could not be opened: {0}")]
  Terminal(io::Error),
  #[error(transparent)]
  Connection(ConnectionError),
  #[error(transparent)]
  Invocation(InvocationError),
}

impl AccountsError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(StoreError::DuplicateAccountName(name)) => {
        Message::new("accounts.errors.nameTaken").with("name", name.clone())
      }
      Self::Store(StoreError::AccountFolderTaken) | Self::Folder(FolderProblem::Taken(_)) => {
        Message::new("accounts.errors.folderTaken")
      }
      Self::Store(error) => messages::store_error(error),
      Self::Folder(FolderProblem::InsideRepository(repository)) => {
        Message::new("accounts.errors.insideRepository").with("repository", repository.display().to_string())
      }
      Self::Folder(FolderProblem::NotAbsolute) => Message::new("accounts.errors.folderNotAbsolute"),
      Self::Folder(FolderProblem::OutsideCredentialPlaces) => Message::new("accounts.errors.folderOutside"),
      Self::CreateFolder(_) => Message::new("accounts.errors.createFolder"),
      Self::NoFolderHere | Self::Unsupported => Message::new("accounts.errors.unsupported"),
      Self::Removal(RemovalError::Default) => Message::new("accounts.errors.removeDefault"),
      Self::Removal(RemovalError::InUse) => Message::new("accounts.errors.inUse"),
      Self::Removal(RemovalError::NotAnotherAccount) => Message::new("accounts.errors.moveTarget"),
      Self::Removal(RemovalError::FolderNotCreated) => Message::new("accounts.errors.notCreated"),
      Self::ProjectsUnreadable(projects) => Message::new("accounts.errors.projectsUnreadable").with(
        "projects",
        projects.iter().map(Name::as_str).collect::<Vec<_>>().join(", "),
      ),
      Self::Trash(_) => Message::new("accounts.errors.trash"),
      Self::CliMissing => Message::new("accounts.errors.cliMissing"),
      Self::FolderMissing(folder) => {
        Message::new("accounts.errors.folderMissing").with("folder", folder.display().to_string())
      }
      Self::Terminal(_) => Message::new("accounts.errors.terminal"),
      Self::Runs(_) | Self::Account(_) | Self::Connection(_) | Self::Invocation(_) => Message::new("errors.storage"),
    }
  }
}
