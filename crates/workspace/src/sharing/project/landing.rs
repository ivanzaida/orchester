//! The invite landing (`docs/architecture/server.md`, "Finding the project"; `xeFRB`, `d8oDsO`): after a project invite
//! is accepted, which local project or checkout to link to the project. The signed-in account's local projects are
//! listed with how each one's `origin` compares with the project's repository hint, and whether it is linked already.
//! One checkout linked to the project, or one that alone matches the hint and is linked to nothing, is opened at once.
//! A folder Orchester doesn't know yet is added as a local project; Orchester never clones a repository.

use std::path::{Path, PathBuf};

use orchester_api::SharedProject;
use orchester_core::{
  id::{ProjectId, ServerId},
  name::Name,
  project::{Project, ProjectKey, TaskKey},
  repository::RepositoryIdentity,
  store::TaskStore,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::link::checkout_origin;
use crate::{
  background,
  project_setup::NewProject,
  sharing::{RemoteMatch, ServerEntry, Servers, SharingError, remote_match},
  workspace::{OpenProject, Workspace},
};

/// A project an invite was to, and the local projects that could be linked to it.
#[derive(Clone, Debug)]
pub struct Landing {
  pub entry: ServerEntry,
  pub project: SharedProject,
  /// The account's local projects: those that match the hint first, those linked to another project last.
  pub checkouts: Vec<Checkout>,
}

/// A local project as a checkout to link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkout {
  pub id: ProjectId,
  pub name: Name,
  pub repo_root: PathBuf,
  pub remote: RemoteMatch,
  /// Its local tasks' keys, by number.
  pub tasks: Vec<TaskKey>,
  pub linked: Option<Linked>,
}

/// What a checkout is linked to already.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Linked {
  /// The project the invite was to.
  This,
  /// Another project, at its address (none for a link made before addresses).
  Other(Option<String>),
}

impl Checkout {
  /// Whether it can be chosen: a checkout links to one project.
  pub fn choosable(&self) -> bool {
    !matches!(self.linked, Some(Linked::Other(_)))
  }

  fn order(&self) -> u8 {
    match (&self.linked, &self.remote) {
      (Some(Linked::This), _) => 0,
      (None, RemoteMatch::Same) => 1,
      (Some(Linked::Other(_)), _) => 3,
      (None, _) => 2,
    }
  }
}

impl Landing {
  /// The checkout opened without the landing: the one linked to the project already, or, when it is the only
  /// candidate that can be picked, the one that matches the project's hint.
  pub fn direct(&self) -> Option<&Checkout> {
    if let Some(here) = self.checkouts.iter().find(|one| one.linked == Some(Linked::This)) {
      return Some(here);
    }
    let mut choosable = self.candidates().filter(|one| one.choosable());
    let one = choosable.next().filter(|one| one.remote == RemoteMatch::Same)?;
    choosable.next().is_none().then_some(one)
  }

  /// The checkouts that could hold the project: linked to it, their `origin` its hint or a fork of it by name, or any
  /// while the project names no repository.
  pub fn candidates(&self) -> impl Iterator<Item = &Checkout> {
    self.checkouts.iter().filter(|checkout| self.candidate(checkout))
  }

  fn candidate(&self, checkout: &Checkout) -> bool {
    let name = |identity: &str| identity.rsplit('/').next().unwrap_or(identity).to_owned();
    match (&checkout.linked, &checkout.remote) {
      (Some(Linked::This), _) | (_, RemoteMatch::Same | RemoteMatch::NoHint) => true,
      (_, RemoteMatch::Different { checkout: origin, hint }) => name(origin.as_str()) == name(hint.as_str()),
      (_, RemoteMatch::NoOrigin) => false,
    }
  }

  /// `git clone` of the project's repository, for a computer without its checkout; `None` without a hint, or for a
  /// hint the command can't hold safely ([`clone_command`]).
  pub fn clone_command(&self) -> Option<String> {
    clone_command(self.project.remote_hint.as_ref()?)
  }
}

/// `git clone https://<identity>.git`, only for an identity made of a plain host (letters, digits, dots and hyphens with
/// an optional port, or a bracketed IPv6 address) and path segments of letters, digits, dots, underscores and hyphens.
/// The hint is the server's word, and a person may paste the command into a shell: anything else (`$(…)`, backticks,
/// `;`, `|`, quotes, control characters) gets no command, and the hint shows as plain text.
pub fn clone_command(identity: &RepositoryIdentity) -> Option<String> {
  let (host, path) = identity.as_str().split_once('/')?;
  let segment = |part: &str| {
    !part.is_empty()
      && part != "."
      && part != ".."
      && part
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
  };
  (plain_host(host) && path.split('/').all(segment)).then(|| format!("git clone https://{}.git", identity.as_str()))
}

/// A host name or IPv4 address with an optional port, or a bracketed IPv6 address with one.
fn plain_host(host: &str) -> bool {
  let (name, port) = match host.strip_prefix('[') {
    Some(rest) => match rest.split_once(']') {
      Some((address, port)) => {
        if address.is_empty()
          || !address
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b':' || byte == b'.')
        {
          return false;
        }
        return port.is_empty() || port.strip_prefix(':').is_some_and(digits);
      }
      None => return false,
    },
    None => host
      .split_once(':')
      .map_or((host, None), |(name, port)| (name, Some(port))),
  };
  !name.is_empty()
    && name
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
    && port.is_none_or(digits)
}

fn digits(text: &str) -> bool {
  !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Why a chosen folder cannot be a checkout to link.
#[derive(Debug, thiserror::Error)]
pub enum CheckoutRefusal {
  /// It isn't the top of a git checkout: a downloaded archive can't hold run branches and worktrees.
  #[error("the folder is not a git checkout")]
  NotGit,
  #[error(transparent)]
  Store(#[from] StoreError),
  /// Work on a background thread ended before it finished; the cause is logged.
  #[error("the work was interrupted")]
  Interrupted,
}

impl From<crate::messages::Message> for CheckoutRefusal {
  /// [`background::run`] fails only when its work did not finish.
  fn from(_: crate::messages::Message) -> Self {
    Self::Interrupted
  }
}

impl Servers {
  /// The project at `slug` on `entry` and the local projects that could be linked to it; `programs` reads their
  /// `origin`. Not a member, or no project there: [`SharingError::NoProject`].
  pub async fn landing(&self, entry: &ServerEntry, slug: &str, programs: &Programs) -> Result<Landing, SharingError> {
    let project = self.find_project(entry, slug).await?;
    let (servers, programs, target) = (self.clone(), programs.clone(), project.clone());
    let server = entry.stored.id;
    let checkouts = background::run(move || servers.checkouts(&programs, server, &target)).await??;
    Ok(Landing {
      entry: entry.clone(),
      project,
      checkouts,
    })
  }

  /// Adds the git checkout in `folder` as a local project, or opens the one it keeps: a new one is named after the
  /// folder. Nothing in the folder changes but Orchester's own data.
  pub async fn add_checkout(&self, folder: PathBuf) -> Result<OpenProject, CheckoutRefusal> {
    let workspace = self.inner.workspace.clone();
    background::run(move || {
      if !folder.join(".git").exists() {
        return Err(CheckoutRefusal::NotGit);
      }
      Ok(open_or_add(&workspace, folder)?)
    })
    .await?
  }

  /// The account's local projects as checkouts for `project` on `server`. Blocking.
  fn checkouts(
    &self,
    programs: &Programs,
    server: ServerId,
    project: &SharedProject,
  ) -> Result<Vec<Checkout>, SharingError> {
    let mut checkouts = Vec::new();
    for registered in self.inner.workspace.projects()? {
      if registered.folder_missing() {
        continue;
      }
      match checkout(programs, &registered.repo_root, (server, project)) {
        Ok(found) => checkouts.push(found),
        Err(error) => tracing::warn!(%error, root = %registered.repo_root.display(), "cannot read a local project"),
      }
    }
    checkouts.sort_by_key(Checkout::order);
    Ok(checkouts)
  }
}

/// The project in `root` as a checkout for `project` on `server`.
fn checkout(
  programs: &Programs,
  root: &Path,
  (server, project): (ServerId, &SharedProject),
) -> Result<Checkout, StoreError> {
  let db = ProjectDb::open(root)?;
  let linked = linked_to(&db, server, project.id)?;
  let mut tasks: Vec<TaskKey> = db.tasks()?.iter().map(|task| task.key().clone()).collect();
  tasks.sort_by_key(TaskKey::number);
  let origin = checkout_origin(programs, root);
  Ok(Checkout {
    id: db.project().id(),
    name: db.project().name().clone(),
    repo_root: root.to_owned(),
    remote: remote_match(origin.as_ref(), project.remote_hint.as_ref()),
    tasks,
    linked,
  })
}

/// What the project in `db` is linked to, seen from `project` on `server`; `None` while it is local.
pub(super) fn linked_to(db: &ProjectDb, server: ServerId, project: ProjectId) -> Result<Option<Linked>, StoreError> {
  Ok(db.sharing()?.map(|link| {
    if link.server == server && link.project == project {
      Linked::This
    } else {
      Linked::Other(link.address)
    }
  }))
}

/// Opens the project `folder` keeps, or adds the folder as a local project named after it. Blocking.
pub(super) fn open_or_add(workspace: &Workspace, folder: PathBuf) -> Result<OpenProject, StoreError> {
  match workspace.open(&folder) {
    Err(StoreError::NotInitialized(_)) => {
      let name = folder_name(&folder);
      let project = Project::new(ProjectId::generate(), ProjectKey::suggest(&name), name);
      workspace.create(NewProject {
        project,
        repo_root: folder,
      })
    }
    opened => opened,
  }
}

/// The name a folder's new project takes: the folder's own, else a word anyone recognises.
fn folder_name(folder: &Path) -> Name {
  folder
    .file_name()
    .and_then(|name| name.to_str())
    .and_then(|name| Name::new(name).ok())
    .unwrap_or_else(|| Name::new("checkout").expect("a fixed name is valid"))
}
