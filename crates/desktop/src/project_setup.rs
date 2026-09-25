//! The local part of first project setup: turning what the user entered into a project and
//! the repository it lives in. The rules themselves are `orchester_core`'s constructors.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  repository::DetectionState,
};
use orchester_store::StoreError;

use crate::messages::{self, Message};

/// Field values of the project form, as entered.
pub struct ProjectDraft<'a> {
  pub name: &'a str,
  pub repository: Option<&'a Path>,
}

/// A validated project, ready to be created in `repo_root`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewProject {
  pub project: Project,
  pub repo_root: PathBuf,
}

/// What keeps the draft from being created.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DraftErrors {
  /// Why the name is not accepted, to show under the name field.
  pub name: Option<Message>,
  pub repository_missing: bool,
}

impl ProjectDraft<'_> {
  /// The key is not asked for: it is suggested from the name and can be changed later.
  pub fn validate(&self) -> Result<NewProject, DraftErrors> {
    match (Name::new(self.name), self.repository) {
      (Ok(name), Some(repository)) => Ok(NewProject {
        project: Project::new(ProjectId::generate(), ProjectKey::suggest(&name), name),
        repo_root: repository.to_path_buf(),
      }),
      (name, repository) => Err(DraftErrors {
        name: name.err().as_ref().map(messages::name_error),
        repository_missing: repository.is_none(),
      }),
    }
  }
}

/// The project Create makes, if it may: the draft is valid and the folder is not known to be
/// outside a git repository (01.C9). GitHub states never hold Create back, and neither does a
/// detection still running (`detection` is `None`).
pub fn creatable(draft: Result<NewProject, DraftErrors>, detection: Option<&DetectionState>) -> Option<NewProject> {
  draft
    .ok()
    .filter(|_| !detection.is_some_and(DetectionState::blocks_project))
}

/// Where a creation error is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorPlace {
  /// Under the repository folder field: the folder itself is the problem.
  Repository,
  /// Beside Create: nothing the user entered is at fault.
  Form,
}

/// Creating the project failed. It stays relevant only while the name and folder it was
/// raised for are unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateFailure {
  pub message: Message,
  pub place: ErrorPlace,
  name: String,
  repo_root: PathBuf,
}

impl lurq::app::component::DevtoolsInspectable for CreateFailure {}

impl CreateFailure {
  pub fn from_store(new: &NewProject, error: &StoreError) -> Self {
    let place = match error {
      StoreError::NotAGitRepository(_) | StoreError::AlreadyInitialized(_) | StoreError::Unrepresentable(_) => {
        ErrorPlace::Repository
      }
      _ => ErrorPlace::Form,
    };
    Self::new(new, messages::store_error(error), place)
  }

  pub fn new(new: &NewProject, message: Message, place: ErrorPlace) -> Self {
    Self {
      message,
      place,
      name: new.project.name().as_str().to_owned(),
      repo_root: new.repo_root.clone(),
    }
  }

  /// Whether the failure still describes `draft`; editing the name or folder retires it.
  pub fn applies_to(&self, draft: &ProjectDraft<'_>) -> bool {
    draft.name.trim() == self.name && draft.repository == Some(self.repo_root.as_path())
  }
}
