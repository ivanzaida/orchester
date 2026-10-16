//! The person's agent CLI accounts (`docs/design/agent-accounts.md`), app-wide in the application database: listing,
//! adding with a new or a brought folder, checking who is signed in, renaming, what uses each one, and removing one
//! as decided (its connections move, it waits while a run is on it, only a folder Orchester created may go to the
//! Recycle Bin / Trash). Opening the CLI's own login is [`sign_in`]. Everything here blocks on SQLite, the file
//! system or a CLI; the UI calls it off the UI thread.

mod error;
mod folders;
pub mod sign_in;
pub mod trash;
mod uses;

use std::path::{Path, PathBuf};

use orchester_core::{
  account::{
    AccountCheck, AccountCli, AccountFolder, AgentAccount, FolderProblem, MoveTo, Removal, account_folder_name,
  },
  connection::{
    ConnectionAccount,
    invocation::{Platform, account_environment, agent_environment},
    plan_limits::LimitReading,
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
  /// Changes whenever an account was added, renamed, checked, moved to another folder or removed through this
  /// workspace or a clone of it: the screens and the status bar that name accounts read them again.
  pub fn account_changes(&self) -> tokio::sync::watch::Receiver<u64> {
    self.account_changes.subscribe()
  }

  fn accounts_changed(&self) {
    self.account_changes.send_modify(|count| *count += 1);
  }

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

  /// What is wrong with `folder` as a brought folder of an account, if anything: it is checked beside every
  /// account of every CLI and each CLI's own folder, which it may neither be, lie in nor hold.
  pub fn brought_folder_problem(&self, folder: &Path) -> Result<Option<FolderProblem>, StoreError> {
    let accounts = self.agent_accounts()?;
    let projects = self.projects()?;
    Ok(folders::brought_problem(
      folder,
      &accounts,
      &projects,
      self.account_root.as_deref(),
    ))
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
        // Nor in a folder brought before the rule that keeps them out of the accounts' folder, nor holding one.
        if let Some(brought) = self.agent_accounts()?.iter().find(|account| {
          matches!(account.folder(), AccountFolder::Brought(brought)
            if folders::overlap(&folders::found(&folder), &folders::found(brought)))
        }) {
          return Err(AccountsError::Folder(FolderProblem::Taken(brought.id())));
        }
        AccountFolder::Created(folder)
      }
      NewFolder::Bring(folder) => {
        if let Some(problem) = self.brought_folder_problem(&folder)? {
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
    self.accounts_changed();
    Ok(account)
  }

  /// Renames the account; everything holding it shows the new name, as they hold its id.
  pub fn rename_agent_account(&self, id: AgentAccountId, name: Name) -> Result<AgentAccount, AccountsError> {
    let mut db = self.db.lock();
    let mut account = db.agent_account(id)?.ok_or(StoreError::AgentAccountNotFound(id))?;
    account.rename(name);
    db.save_agent_account(&account)?;
    drop(db);
    self.accounts_changed();
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
    drop(db);
    self.accounts_changed();
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
    if let Some(folder) = trash {
      self.trashable(&account, folder, &accounts)?;
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
    self.accounts_changed();
    // The account is removed by now: a refusal says so and where its folder stays.
    if let Some(folder) = trash
      && let Err(error) = self.trash.take(folder)
    {
      return Err(AccountsError::TrashFailed {
        folder: folder.to_path_buf(),
        error,
      });
    }
    Ok(())
  }

  /// Refused unless `folder`, `account`'s, may go to the trash: this window has one, it is still the folder Orchester
  /// made there (not a link or a junction now, nor reached through one), and it neither holds nor lies in another
  /// account's folder or a CLI's own folder. Checked before anything is removed.
  fn trashable(&self, account: &AgentAccount, folder: &Path, accounts: &[AgentAccount]) -> Result<(), AccountsError> {
    if !self.trash.is_set() {
      return Err(AccountsError::NoTrash);
    }
    if !folders::still_created_here(folder) {
      return Err(AccountsError::Removal(
        orchester_core::account::RemovalError::FolderNotCreated,
      ));
    }
    let mut others = accounts
      .iter()
      .filter(|other| other.id() != account.id())
      .filter_map(|other| match other.folder().path() {
        Some(path) => Some(path.to_path_buf()),
        None => folders::own_folder(other.cli()),
      });
    if others.any(|other| folders::overlap(&folders::found(&other), &folders::found(folder))) {
      return Err(AccountsError::HoldsAnotherAccount);
    }
    Ok(())
  }

  /// Keeps the account in `folder`, a folder the person already has, after its own went missing (*Choose folder…*,
  /// `E96CI`): the folder rules of a brought folder apply (beside every other account), the connections of every
  /// listed project follow (a project out of reach follows when it opens), and the account is checked there.
  pub fn choose_agent_account_folder(&self, id: AgentAccountId, folder: &Path) -> Result<AgentAccount, AccountsError> {
    let mut account = self.agent_account(id)?.ok_or(StoreError::AgentAccountNotFound(id))?;
    let others: Vec<AgentAccount> = self
      .agent_accounts()?
      .into_iter()
      .filter(|other| other.id() != id)
      .collect();
    let projects = self.projects()?;
    if let Some(problem) = folders::brought_problem(folder, &others, &projects, self.account_root.as_deref()) {
      return Err(AccountsError::Folder(problem));
    }
    account
      .move_to_folder(folders::found(folder))
      .map_err(AccountsError::Account)?;
    self.db.lock().save_agent_account_folder(&account)?;
    self.accounts_changed();
    uses::move_connections(&account, &ConnectionAccount::of(&account), &projects)?;
    self.check_agent_account(id)
  }

  /// The last report of the account's plan limits in any listed project, as its CLI gave it during a run: what the
  /// account's details show before this session's runs report. A project that can't be read is passed over.
  pub fn last_plan_reading(&self, id: AgentAccountId) -> Result<Option<LimitReading>, AccountsError> {
    let mut last: Option<LimitReading> = None;
    for project in self.projects()?.iter().filter(|project| !project.folder_missing()) {
      let reading = orchester_store::ProjectDb::open(&project.repo_root).and_then(|db| db.last_account_reading(id));
      match reading {
        Ok(Some(reading)) if last.as_ref().is_none_or(|known| known.at < reading.at) => last = Some(reading),
        Ok(_) => {}
        Err(error) => {
          tracing::warn!(%error, project = %project.repo_root.display(), "cannot read an account's plan limits")
        }
      }
    }
    Ok(last)
  }

  /// [`Self::last_plan_reading`] of every account that has one.
  pub fn last_plan_readings(&self) -> Result<Vec<LimitReading>, AccountsError> {
    let mut readings = Vec::new();
    for account in self.agent_accounts()? {
      readings.extend(self.last_plan_reading(account.id())?);
    }
    Ok(readings)
  }

  /// Brings the connections of `repo_root` up to date with the accounts: one on a removed account goes where its
  /// removal sent the others, one on an account whose folder moved gets the new folder. For a project that could not
  /// be reached then; called when a project opens.
  pub fn follow_accounts(&self, repo_root: &Path) -> Result<(), AccountsError> {
    let removed = self.db.lock().removed_agent_accounts()?;
    let accounts = self.agent_accounts()?;
    uses::follow_accounts(repo_root, &removed, &accounts)
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
