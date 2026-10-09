use std::path::{Path, PathBuf};

use orchester_core::name::Name;
use orchester_store::{RegisteredProject, StoreError};
use orchester_workspace::{
  messages::{self, Message},
  workspace::OpenProject,
};

/// What a project id stands for in the workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectState {
  Open(OpenProject),
  Unavailable(Unavailable),
}

/// Why a project cannot be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unavailable {
  /// No project in the list has the id: it was removed from the list, or the address was made up.
  Unknown,
  /// The project is in the list, but its folder is not where it was: moved, renamed or deleted.
  FolderMissing { name: Name, repo_root: PathBuf },
  /// The project's folder is there, but its data cannot be opened, and why; `newer_data` says that a
  /// newer Orchester wrote it.
  Unreadable {
    name: Name,
    repo_root: PathBuf,
    reason: Message,
    newer_data: bool,
  },
  /// The list of projects itself cannot be read, and why.
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for ProjectState {}

impl ProjectState {
  /// The project's name, when it is in the list.
  pub fn name(&self) -> Option<&Name> {
    match self {
      Self::Open(open) => Some(open.project.name()),
      Self::Unavailable(Unavailable::FolderMissing { name, .. } | Unavailable::Unreadable { name, .. }) => Some(name),
      Self::Unavailable(Unavailable::Unknown | Unavailable::Failed(_)) => None,
    }
  }

  /// The project's folder, when it is in the list.
  pub fn repo_root(&self) -> Option<&Path> {
    match self {
      Self::Open(open) => Some(&open.repo_root),
      Self::Unavailable(unavailable) => unavailable.repo_root(),
    }
  }

  /// The name `app_state` reports the state by.
  pub const fn kind(&self) -> &'static str {
    match self {
      Self::Open(_) => "open",
      Self::Unavailable(Unavailable::Unknown) => "unknown",
      Self::Unavailable(Unavailable::FolderMissing { .. }) => "folder_missing",
      Self::Unavailable(Unavailable::Unreadable { .. }) => "unreadable",
      Self::Unavailable(Unavailable::Failed(_)) => "failed",
    }
  }

  pub fn open(&self) -> Option<&OpenProject> {
    match self {
      Self::Open(open) => Some(open),
      Self::Unavailable(_) => None,
    }
  }
}

impl Unavailable {
  /// `project`, which the workspace could not open because of `error`.
  pub(super) fn of(project: RegisteredProject, error: &StoreError) -> Self {
    if project.folder_missing() {
      return Self::FolderMissing {
        name: project.name,
        repo_root: project.repo_root,
      };
    }
    tracing::warn!(%error, repo_root = %project.repo_root.display(), "cannot open a project");
    Self::Unreadable {
      name: project.name,
      repo_root: project.repo_root,
      reason: messages::store_error(error),
      newer_data: matches!(error, StoreError::SchemaTooNew { .. }),
    }
  }

  pub fn repo_root(&self) -> Option<&Path> {
    match self {
      Self::FolderMissing { repo_root, .. } | Self::Unreadable { repo_root, .. } => Some(repo_root),
      Self::Unknown | Self::Failed(_) => None,
    }
  }
}
