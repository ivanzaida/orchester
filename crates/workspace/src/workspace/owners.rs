//! Moving a project between this computer and the signed-in Orchester Cloud account (the person's decision,
//! 2026-10-30). A person does it on All projects, only for a project that is not shared: a project and its server
//! always have the same owner, and each shared project syncs through its own owner's server. Nothing about the
//! project's folder or its data changes, only whose it is.

use orchester_core::id::{CloudAccountId, ProjectId, ServerId};
use orchester_store::{AppDb, ProjectDb, RegisteredProject, StoreError};

use super::{Workspace, owner};
use crate::sharing::SharingError;

/// Where a project moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveTo {
  /// The signed-in account's: it then shows only while that account is signed in here.
  Account,
  /// This computer's: it shows to whoever is signed in, and to nobody signed in.
  Computer,
}

/// Why a project did not move; nothing was changed.
#[derive(Debug, thiserror::Error)]
pub enum MoveError {
  /// Nobody is signed in, so there is no account to move it into or out of.
  #[error("nobody is signed in")]
  SignedOut,
  /// It is shared, or being shared now: it stays with its server until it is made local.
  #[error("a shared project stays where it is")]
  Shared,
  /// Its folder is gone, so whether it is shared cannot be told.
  #[error("the project's folder is missing")]
  FolderMissing,
  #[error(transparent)]
  Store(#[from] StoreError),
}

/// Whether `named` names `project`, as a tool names a project: by its id, or by its name ignoring case.
fn named_by(project: &RegisteredProject, named: &str) -> bool {
  project.id.to_string() == named || project.name.as_str().to_lowercase() == named.to_lowercase()
}

/// The account a first link of `project` to `server` moves it into: a project of this computer's goes into the account
/// whose server it is. An account's is refused a server of this computer's ([`SharingError::AccountProject`]): the
/// person moves it, and only they know it should show to everyone on this computer. One not known yet has no row to
/// change.
fn moved_by_link(db: &AppDb, project: ProjectId, server: ServerId) -> Result<Option<CloudAccountId>, SharingError> {
  let Some(server_owner) = db.server_owner(server)? else {
    return Err(SharingError::ServerMissing);
  };
  let Some(project_owner) = db.project_owner(project)? else {
    return Ok(None);
  };
  match (project_owner, server_owner) {
    (None, Some(account)) => Ok(Some(account)),
    (Some(_), None) => Err(SharingError::AccountProject),
    (Some(held), Some(account)) if held != account => Err(StoreError::ProjectOfAnotherAccount(project).into()),
    _ => Ok(None),
  }
}

impl Workspace {
  /// Moves `project` as a person asked. A folder that cannot be read is refused, not taken for unshared.
  pub(crate) fn move_project(&self, project: &RegisteredProject, to: MoveTo) -> Result<(), MoveError> {
    if project.folder_missing() {
      return Err(MoveError::FolderMissing);
    }
    if ProjectDb::open(&project.repo_root)?.sharing()?.is_some() {
      return Err(MoveError::Shared);
    }
    let mut db = self.db.lock();
    let Some(account) = owner(&db)? else {
      return Err(MoveError::SignedOut);
    };
    let target = match to {
      MoveTo::Account => Some(&account),
      MoveTo::Computer => None,
    };
    if db.set_project_owner(project.id, target, &account)? {
      Ok(())
    } else {
      // It left the list meanwhile, or another account holds it: nothing here may move it.
      Err(MoveError::Store(StoreError::ProjectOfAnotherAccount(project.id)))
    }
  }

  /// The projects that wait for an account to sign in while nobody is signed in; none while one is, since another
  /// account's stay out of sight.
  pub fn waiting_projects(&self) -> Result<Vec<RegisteredProject>, StoreError> {
    let db = self.db.lock();
    if owner(&db)?.is_some() {
      return Ok(Vec::new());
    }
    db.projects_of_accounts()
  }

  /// The servers a first link to which moves `project` into the signed-in account, as the share page says: the
  /// account's own, while the project is this computer's.
  pub fn servers_moving_project(&self, project: ProjectId) -> Result<Vec<ServerId>, StoreError> {
    let db = self.db.lock();
    let (Some(account), Some(None)) = (owner(&db)?, db.project_owner(project)?) else {
      return Ok(Vec::new());
    };
    Ok(
      db.servers_of(Some(&account))?
        .into_iter()
        .map(|server| server.id)
        .collect(),
    )
  }

  /// The name and address of `server`, for an invite landing that asks a sign-in first: it says what the invite is to
  /// on this computer, whoever's the server is, since the invite names it anyway.
  pub fn server_named(&self, server: ServerId) -> Result<Option<String>, StoreError> {
    Ok(
      self
        .db
        .lock()
        .every_server()?
        .into_iter()
        .find(|known| known.id == server)
        .map(|known| known.name),
    )
  }

  /// Whether a project `named`, by its id or its name, waits for its account while nobody is signed in: what
  /// [`crate::app_mcp`] answers instead of *not found*. Signed in, another account's stays not found.
  pub(crate) fn held_by_an_account(&self, named: &str) -> Result<bool, StoreError> {
    Ok(self.waiting_projects()?.iter().any(|project| named_by(project, named)))
  }

  /// Refuses a first link of `project` to `server` before anything is sent, when their owners cannot meet.
  pub fn check_link_owner(&self, project: ProjectId, server: ServerId) -> Result<(), SharingError> {
    moved_by_link(&self.db.lock(), project, server).map(drop)
  }

  /// Writes a first link of `project` to `server` with `write`, once the server accepted it, the project taking the
  /// server's owner first so the two never differ while the link exists. A link that is not written leaves the project
  /// whose it was: a local project moves into an account only when it is shared there.
  pub fn link_with_owner<T>(
    &self,
    project: ProjectId,
    server: ServerId,
    write: impl FnOnce() -> Result<T, SharingError>,
  ) -> Result<T, SharingError> {
    let moved = {
      let mut db = self.db.lock();
      let moved = moved_by_link(&db, project, server)?;
      if let Some(account) = &moved {
        db.set_project_owner(project, Some(account), account)?;
      }
      moved
    };
    write().inspect_err(|_| {
      if let Some(account) = &moved
        && let Err(error) = self.db.lock().set_project_owner(project, None, account)
      {
        tracing::error!(%error, %project, "a project stays in the account after a link that was not written");
      }
    })
  }

  /// Gives a project just added for a first link to `server` the server's owner: it holds nothing yet that anyone else
  /// could see by surprise, and a server of this computer's takes only this computer's projects.
  pub(crate) fn give_new_project_to_server_owner(
    &self,
    project: ProjectId,
    server: ServerId,
  ) -> Result<(), StoreError> {
    let mut db = self.db.lock();
    let (Some(account), Some(server_owner)): (Option<CloudAccountId>, _) = (owner(&db)?, db.server_owner(server)?)
    else {
      return Ok(());
    };
    db.set_project_owner(project, server_owner.as_ref(), &account)?;
    Ok(())
  }
}
