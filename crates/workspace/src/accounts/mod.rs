//! The person's agent CLI accounts (`docs/design/agent-accounts.md`), app-wide in the application database: listing,
//! adding with a new or a brought folder, checking who is signed in, renaming, what uses each one, and removing one
//! as decided (its connections move, it waits while a run is on it, only a folder Orchester created may go to the
//! Recycle Bin / Trash). Opening the CLI's own login is [`sign_in`]. Everything here blocks on SQLite, the file
//! system or a CLI; the UI calls it off the UI thread.

mod error;
mod folders;
pub mod sign_in;
mod uses;

use std::path::{Path, PathBuf};

use orchester_core::{
  account::{
    AccountCheck, AccountCli, AccountFolder, AgentAccount, FolderProblem, MoveTo, Removal, account_folder_name,
  },
  connection::{
    ConnectionAccount,
    invocation::{Platform, account_environment, agent_environment},
  },
  id::AgentAccountId,
  name::Name,
  time::Timestamp,
};
use orchester_exec::ProcessRunner;
use orchester_store::StoreError;

pub use self::{
  error::AccountsError,
  folders::own_folder,
  uses::{AccountUses, ConnectionUse, RunUse},
};
use crate::{connections::probe, workspace::Workspace};

/// A CLI that does not answer its status command within this time counts as not found.
const CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Where a new account's folder comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NewFolder {
  /// A new folder Orchester creates in its application data.
  Create,
  /// A folder the person already uses.
  Bring(PathBuf),
}

/// An account as *Add account* saves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewAccount {
  pub cli: AccountCli,
  pub name: Name,
  pub folder: NewFolder,
}

impl Workspace {
  /// The accounts, by CLI, Default first.
  pub fn agent_accounts(&self) -> Result<Vec<AgentAccount>, StoreError> {
    self.db.lock().agent_accounts()
  }

  /// The account, removed or not: a run still names the account it ran on.
  pub fn agent_account(&self, id: AgentAccountId) -> Result<Option<AgentAccount>, StoreError> {
    self.db.lock().agent_account(id)
  }

  /// The folder Orchester would create for an account of `cli` named `name`; `None` where accounts have no folder
  /// of Orchester's on this platform.
  pub fn planned_folder(&self, cli: AccountCli, name: &str) -> Option<PathBuf> {
    let root = self.account_root.as_deref()?;
    Some(root.join(account_folder_name(cli.program(), name, |candidate| {
      root.join(candidate).exists()
    })))
  }

  /// What is wrong with `folder` as a brought folder of an account of `cli`, if anything.
  pub fn brought_folder_problem(&self, cli: AccountCli, folder: &Path) -> Result<Option<FolderProblem>, StoreError> {
    let accounts = self.cli_accounts(cli)?;
    let projects = self.projects()?;
    Ok(folders::brought_problem(cli, folder, &accounts, &projects))
  }

  /// Adds an account, creating its folder or taking the person's, not checked yet: *Add account* checks a brought
  /// folder at once and a new one after the sign-in.
  pub fn add_agent_account(&self, new: NewAccount) -> Result<AgentAccount, AccountsError> {
    if !new.cli.supports_accounts_on(Platform::current()) {
      return Err(AccountsError::Unsupported);
    }
    let folder = match new.folder {
      NewFolder::Create => {
        let folder = self
          .planned_folder(new.cli, new.name.as_str())
          .ok_or(AccountsError::NoFolderHere)?;
        // Where accounts' folders go may be named (`ORCHESTER_ACCOUNTS_DIR`): never inside a repository either.
        if let Some(repository) = folders::repository_of(&folders::found(&folder), &self.projects()?) {
          return Err(AccountsError::Folder(FolderProblem::InsideRepository(repository)));
        }
        AccountFolder::Created(folder)
      }
      NewFolder::Bring(folder) => {
        if let Some(problem) = self.brought_folder_problem(new.cli, &folder)? {
          return Err(AccountsError::Folder(problem));
        }
        AccountFolder::Brought(folders::found(&folder))
      }
    };
    let account = AgentAccount::new(AgentAccountId::generate(), new.cli, new.name, folder, Timestamp::now())
      .map_err(AccountsError::Account)?;
    let made = match account.folder() {
      AccountFolder::Created(folder) => {
        std::fs::create_dir_all(folder).map_err(AccountsError::CreateFolder)?;
        Some(folder)
      }
      AccountFolder::Own | AccountFolder::Brought(_) => None,
    };
    if let Err(error) = self.db.lock().add_agent_account(&account) {
      // The folder was made for this account only, and is empty: refused (a name taken), it goes again.
      if let Some(folder) = made
        && let Err(removed) = std::fs::remove_dir(folder)
      {
        tracing::warn!(%removed, folder = %folder.display(), "an unused account folder stays");
      }
      return Err(error.into());
    }
    Ok(account)
  }

  /// Renames the account; everything holding it shows the new name, as they hold its id.
  pub fn rename_agent_account(&self, id: AgentAccountId, name: Name) -> Result<AgentAccount, AccountsError> {
    let mut db = self.db.lock();
    let mut account = db.agent_account(id)?.ok_or(StoreError::AgentAccountNotFound(id))?;
    account.rename(name);
    db.save_agent_account(&account)?;
    Ok(account)
  }

  /// Asks the account's CLI who is signed in to its folder and records the answer. The CLI runs from the `PATH`,
  /// with the environment agents get and the account's folder variable.
  pub fn check_agent_account(&self, id: AgentAccountId) -> Result<AgentAccount, AccountsError> {
    let account = self.agent_account(id)?.ok_or(StoreError::AgentAccountNotFound(id))?;
    let check = check(&account)?;
    let mut db = self.db.lock();
    // Read again: the person may have renamed it while the CLI answered.
    let mut account = db.agent_account(id)?.ok_or(StoreError::AgentAccountNotFound(id))?;
    account.record_check(check);
    db.save_agent_account(&account)?;
    Ok(account)
  }

  /// The connections of every listed project that run on the account, with their profiles, and the runs on it now.
  pub fn agent_account_uses(&self, id: AgentAccountId) -> Result<AccountUses, AccountsError> {
    let account = self.agent_account(id)?.ok_or(StoreError::AgentAccountNotFound(id))?;
    Ok(uses::uses(&account, &self.projects()?))
  }

  /// Removes the account as `move_to` and `trash` say: its connections in every listed project move first, then
  /// the account leaves the list, then its folder goes to the Recycle Bin / Trash if asked. Refused for Default,
  /// while a run that has not ended is on it, while a listed project can't be read (a run there may be on it), and
  /// for a folder Orchester did not create.
  pub fn remove_agent_account(&self, id: AgentAccountId, move_to: MoveTo, trash: bool) -> Result<(), AccountsError> {
    let account = self.agent_account(id)?.ok_or(StoreError::AgentAccountNotFound(id))?;
    let accounts = self.agent_accounts()?;
    let projects = self.projects()?;
    let uses = uses::uses(&account, &projects);
    if !uses.unreadable.is_empty() {
      return Err(AccountsError::ProjectsUnreadable(uses.unreadable));
    }
    let in_use = !uses.running.is_empty();
    let removal = Removal::plan(&account, move_to, trash, &accounts, in_use).map_err(AccountsError::Removal)?;
    // A folder that is gone leaves nothing to move to the bin; one that is no longer the folder Orchester made
    // there (a link or a junction now, or reached through one) is not moved.
    let trash = removal.trash().filter(|folder| folder.exists());
    if trash.is_some_and(|folder| !folders::still_created_here(folder)) {
      return Err(AccountsError::Removal(
        orchester_core::account::RemovalError::FolderNotCreated,
      ));
    }
    let target = match removal.move_to() {
      MoveTo::Account(target) => accounts
        .iter()
        .find(|other| other.id() == target)
        .map(ConnectionAccount::of),
      MoveTo::NoAccount => None,
    }
    .unwrap_or(ConnectionAccount::Unassigned);
    uses::move_connections(&account, &target, &projects)?;
    let moved_to = match removal.move_to() {
      MoveTo::Account(target) => Some(target),
      MoveTo::NoAccount => None,
    };
    self.db.lock().remove_agent_account(id, moved_to, Timestamp::now())?;
    if let Some(folder) = trash {
      orchester_exec::trash::move_to_trash(folder).map_err(AccountsError::Trash)?;
    }
    Ok(())
  }

  /// Moves the connections of `repo_root` still on a removed account to where its removal sent the others: a
  /// project that could not be reached when the account was removed. Called when a project opens.
  pub fn follow_removed_accounts(&self, repo_root: &Path) -> Result<(), AccountsError> {
    let removed = self.db.lock().removed_agent_accounts()?;
    if removed.is_empty() {
      return Ok(());
    }
    let accounts = self.agent_accounts()?;
    uses::follow_removals(repo_root, &removed, &accounts)
  }

  fn cli_accounts(&self, cli: AccountCli) -> Result<Vec<AgentAccount>, StoreError> {
    Ok(
      self
        .agent_accounts()?
        .into_iter()
        .filter(|account| account.cli() == cli)
        .collect(),
    )
  }
}

/// Checks `account` now.
fn check(account: &AgentAccount) -> Result<AccountCheck, AccountsError> {
  let now = Timestamp::now();
  if account.folder().path().is_some_and(|folder| !folder.is_dir()) {
    return Ok(AccountCheck::folder_missing(now));
  }
  let provider = account.cli().provider();
  let environment = account_environment(
    agent_environment(std::env::vars_os()),
    &ConnectionAccount::of(account),
    provider,
  )
  .map_err(AccountsError::Invocation)?;
  let program = probe::detect(account.cli().program()).into_iter().next();
  Ok(AccountCheck::run(
    account.cli(),
    program.as_deref(),
    &ProcessRunner::new(CHECK_TIMEOUT),
    &environment,
    now,
  ))
}
