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
  store::TaskStore,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::link::checkout_origin;
use crate::{
  background,
  project_setup::NewProject,
  sharing::{RemoteMatch, ServerEntry, Servers, SharingError, remote_match},
  workspace::OpenProject,
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

  /// `git clone` of the project's repository, for a computer without its checkout; `None` without a hint.
  pub fn clone_command(&self) -> Option<String> {
    let hint = self.project.remote_hint.as_ref()?;
    Some(format!("git clone https://{}.git", hint.as_str()))
  }
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
      match workspace.open(&folder) {
        Ok(open) => Ok(open),
        Err(StoreError::NotInitialized(_)) => {
          let name = folder_name(&folder);
          let project = Project::new(ProjectId::generate(), ProjectKey::suggest(&name), name);
          Ok(workspace.create(NewProject {
            project,
            repo_root: folder,
          })?)
        }
        Err(error) => Err(error.into()),
      }
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
fn checkout(programs: &Programs, root: &Path, (server, project): (ServerId, &SharedProject)) -> Result<Checkout, StoreError> {
  let db = ProjectDb::open(root)?;
  let linked = db.sharing()?.map(|link| {
    if link.server == server && link.project == project.id {
      Linked::This
    } else {
      Linked::Other(link.address)
    }
  });
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

/// The name a folder's new project takes: the folder's own, else a word anyone recognises.
fn folder_name(folder: &Path) -> Name {
  folder
    .file_name()
    .and_then(|name| name.to_str())
    .and_then(|name| Name::new(name).ok())
    .unwrap_or_else(|| Name::new("checkout").expect("a fixed name is valid"))
}
