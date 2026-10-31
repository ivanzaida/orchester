//! Opening a team's shared project from the start page (`docs/design/startup.md`, decision 6): the folder the person
//! picked is checked first — a git checkout whose `origin` is the project's repository hint, by [`remote_match`]'s
//! rule, and linked to no other project — then linked to the project as *Join* links a checkout
//! ([`Servers::link_plan`], [`Servers::link_checkout`]). A different `origin` is linked only when the person says to
//! link it anyway.

use std::{fs, path::Path};

use orchester_api::SharedProject;
use orchester_core::{
  id::{IdempotencyKey, ServerId},
  name::Name,
  permission::stays_local,
  repository::RepositoryIdentity,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::{
  IdentityProblem, JoinReview, ShareError,
  first_link::{LinkKind, LinkRequest},
  landing::{Linked, add_for_link, linked_to, open_or_add},
  repository_identity,
};
use crate::{
  background,
  messages::Message,
  sharing::{RemoteMatch, ServerEntry, Servers, SharingError, remote_match},
  workspace::OpenProject,
};

/// What a folder is, as a checkout of a shared project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderCheck {
  /// Its `origin` is the project's repository hint.
  Matches,
  /// The project names no repository: nothing to compare.
  NoHint,
  /// Its `origin` is another repository than the project's hint: it is linked only anyway.
  Differs {
    origin: RepositoryIdentity,
    hint: RepositoryIdentity,
  },
  /// It has no `origin`, or one that is a folder on this computer.
  NoOrigin,
  /// The path is not absolute: it would name a folder relative to wherever Orchester runs. Nothing was read.
  NotAbsolute,
  /// The path names another machine, a device or a drive mapped to a network share. Nothing was read through it.
  NotLocal,
  /// It has no `.git` at its top.
  NotRepository,
  /// The folder, or its git remotes, can't be read.
  Unreadable,
  /// It is linked to this project already.
  LinkedHere,
  /// It is linked to another project, `name` here, at `address` (none for a link made before addresses).
  LinkedElsewhere { name: Name, address: Option<String> },
}

impl FolderCheck {
  /// Whether the folder can't be linked whatever the person says.
  pub const fn refuses(&self) -> bool {
    matches!(
      self,
      Self::NotAbsolute | Self::NotLocal | Self::NotRepository | Self::Unreadable | Self::LinkedElsewhere { .. }
    )
  }

  /// What the check found, in words, for `project`.
  pub fn message(&self, project: &SharedProject) -> Message {
    let hint = || {
      project
        .remote_hint
        .as_ref()
        .map_or_else(String::new, |hint| hint.as_str().to_owned())
    };
    let named = project.name.clone();
    match self {
      Self::Matches => Message::new("sharing.folder.matches").with("hint", hint()),
      Self::NoHint => Message::new("sharing.folder.noHint").with("project", named),
      Self::Differs { origin, hint } => Message::new("sharing.folder.differs")
        .with("origin", origin.as_str().to_owned())
        .with("project", named)
        .with("hint", hint.as_str().to_owned()),
      Self::NoOrigin => Message::new("sharing.folder.noOrigin").with("hint", hint()),
      Self::NotAbsolute => Message::new("sharing.folder.notAbsolute"),
      Self::NotLocal => Message::new("sharing.folder.notLocal"),
      Self::NotRepository => Message::new("sharing.folder.notRepository"),
      Self::Unreadable => Message::new("sharing.folder.unreadable"),
      Self::LinkedHere => Message::new("sharing.folder.linkedHere").with("project", named),
      Self::LinkedElsewhere { name, .. } => {
        Message::new("sharing.folder.linkedElsewhere").with("name", name.as_str().to_owned())
      }
    }
  }
}

/// The folder linked, or what is left to decide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderLinked {
  /// Linked now, or linked to this project already: open it.
  Open(OpenProject),
  /// The folder's project and the server project both have tasks: nothing is linked yet; the person reviews the join
  /// and [`Servers::join`] finishes it.
  Review(JoinReview),
}

/// Why the folder was not linked.
#[derive(Debug, thiserror::Error)]
pub enum FolderLinkError {
  /// The check refused the folder; nothing was changed.
  #[error("the folder can't be linked to this project")]
  Refused(FolderCheck),
  #[error(transparent)]
  Share(#[from] ShareError),
}

impl From<SharingError> for FolderLinkError {
  fn from(error: SharingError) -> Self {
    Self::Share(error.into())
  }
}

impl From<StoreError> for FolderLinkError {
  fn from(error: StoreError) -> Self {
    Self::Share(error.into())
  }
}

/// How the check opens the folder's project database.
#[derive(Clone, Copy)]
enum Opening {
  /// As it is: nothing in the folder is written, so a field that changes, or automation, can check any folder.
  ReadOnly,
  /// Migrated first, as opening the project does: only when the person links it.
  Migrating,
}

/// What `folder` is as a checkout of `project` on `server`, read without writing anything in it. A path that is not
/// absolute, or that names another machine, is refused by its text before anything reads it: reading
/// `\\server\share` or a drive mapped to one makes Windows contact that server with the person's credentials. A
/// project database that can't be read is an error, not a check. Blocking.
pub fn check_folder(
  programs: &Programs,
  (server, project): (ServerId, &SharedProject),
  folder: &Path,
) -> Result<FolderCheck, StoreError> {
  check_with(programs, (server, project), folder, Opening::ReadOnly)
}

fn check_with(
  programs: &Programs,
  (server, project): (ServerId, &SharedProject),
  folder: &Path,
  opening: Opening,
) -> Result<FolderCheck, StoreError> {
  if !folder.is_absolute() {
    return Ok(FolderCheck::NotAbsolute);
  }
  if let Err(error) = stays_local(folder) {
    tracing::warn!(%error, "a folder chosen for a shared project is not on this computer");
    return Ok(FolderCheck::NotLocal);
  }
  match fs::metadata(folder) {
    Ok(metadata) if metadata.is_dir() => {}
    Ok(_) => return Ok(FolderCheck::Unreadable),
    Err(error) => {
      tracing::warn!(%error, folder = %folder.display(), "cannot read a folder chosen for a shared project");
      return Ok(FolderCheck::Unreadable);
    }
  }
  match folder.join(".git").try_exists() {
    Ok(true) => {}
    Ok(false) => return Ok(FolderCheck::NotRepository),
    Err(error) => {
      tracing::warn!(%error, folder = %folder.display(), "cannot read a chosen folder's .git");
      return Ok(FolderCheck::Unreadable);
    }
  }
  if let Some(linked) = linked_check(folder, (server, project), opening)? {
    return Ok(linked);
  }
  let origin = match repository_identity(programs, folder) {
    Ok(identity) => Some(identity),
    Err(IdentityProblem::NoOrigin | IdentityProblem::LocalOrigin { .. }) => None,
    Err(IdentityProblem::Unreadable) => return Ok(FolderCheck::Unreadable),
  };
  Ok(match remote_match(origin.as_ref(), project.remote_hint.as_ref()) {
    RemoteMatch::Same => FolderCheck::Matches,
    RemoteMatch::Different { checkout, hint } => FolderCheck::Differs { origin: checkout, hint },
    RemoteMatch::NoHint => FolderCheck::NoHint,
    RemoteMatch::NoOrigin => FolderCheck::NoOrigin,
  })
}

/// Whether `folder`'s project is linked to `project` or to another one; `None` when it has no project, or none linked.
/// Read [`Opening::ReadOnly`], a database this build has not migrated yet can't be read without writing it: its links
/// are read again when the person links the folder, which migrates it.
fn linked_check(
  folder: &Path,
  (server, project): (ServerId, &SharedProject),
  opening: Opening,
) -> Result<Option<FolderCheck>, StoreError> {
  let linked = |db: &ProjectDb| -> Result<Option<FolderCheck>, StoreError> {
    Ok(match linked_to(db, server, project.id)? {
      Some(Linked::This) => Some(FolderCheck::LinkedHere),
      Some(Linked::Other(address)) => Some(FolderCheck::LinkedElsewhere {
        name: db.project().name().clone(),
        address,
      }),
      None => None,
    })
  };
  match opening {
    Opening::ReadOnly => match ProjectDb::open_read_only(folder) {
      Err(StoreError::NotInitialized(_) | StoreError::SchemaTooOld { .. }) => Ok(None),
      Err(error) => Err(error),
      Ok(db) => linked(&db),
    },
    Opening::Migrating => match ProjectDb::open(folder) {
      Err(StoreError::NotInitialized(_)) => Ok(None),
      Err(error) => Err(error),
      Ok(db) => linked(&db),
    },
  }
}

impl Servers {
  /// Links `folder` to `project` on `entry`, as [`check_folder`] allows: a different `origin` only `anyway`. The check
  /// runs again here, its project database migrated, since this is the person's press. Its `.orchester` is set up
  /// first, so a server that fails afterwards leaves it a local project. `key` is the upload's idempotency key, made
  /// when the person chose the folder: a retry links once.
  pub async fn link_folder(
    &self,
    entry: &ServerEntry,
    project: &SharedProject,
    folder: &Path,
    programs: &Programs,
    (anyway, key): (bool, IdempotencyKey),
  ) -> Result<FolderLinked, FolderLinkError> {
    let (programs, target, root, server) = (programs.clone(), project.clone(), folder.to_owned(), entry.stored.id);
    let check = background::run(move || check_with(&programs, (server, &target), &root, Opening::Migrating))
      .await
      .map_err(SharingError::from)??;
    // The `origin` the server project takes as its hint when it has none, as the Join page sends it.
    let remote = match check {
      FolderCheck::NotAbsolute
      | FolderCheck::NotLocal
      | FolderCheck::NotRepository
      | FolderCheck::Unreadable
      | FolderCheck::LinkedElsewhere { .. } => return Err(FolderLinkError::Refused(check)),
      FolderCheck::Differs { .. } if !anyway => return Err(FolderLinkError::Refused(check)),
      FolderCheck::LinkedHere => return Ok(FolderLinked::Open(self.open_folder(folder).await?)),
      FolderCheck::Matches => project.remote_hint.clone(),
      FolderCheck::Differs { origin, .. } => Some(origin),
      FolderCheck::NoHint | FolderCheck::NoOrigin => None,
    };
    self.add_for_link(folder, server).await?;
    let plan = self.link_plan(entry, folder, project.clone()).await?;
    if let LinkKind::Review { .. } = plan.kind {
      return Ok(FolderLinked::Review(self.review(entry, folder, project.clone()).await?));
    }
    let request = LinkRequest {
      remote,
      key,
      write_discovery: false,
    };
    self.link_checkout(entry, folder, &plan, request).await?;
    Ok(FolderLinked::Open(self.open_folder(folder).await?))
  }

  /// The project `folder` keeps, registered, or the folder added as a local project.
  async fn open_folder(&self, folder: &Path) -> Result<OpenProject, SharingError> {
    let (workspace, root) = (self.inner.workspace.clone(), folder.to_owned());
    Ok(background::run(move || open_or_add(&workspace, root)).await??)
  }

  /// [`Self::open_folder`] for a first link to `server`: a project added for it now has the server's owner.
  async fn add_for_link(&self, folder: &Path, server: ServerId) -> Result<(), SharingError> {
    let (workspace, root) = (self.inner.workspace.clone(), folder.to_owned());
    background::run(move || add_for_link(&workspace, root, server)).await??;
    Ok(())
  }
}
