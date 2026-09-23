//! The local part of first project setup: turning what the user entered into a project and
//! the repository it lives in. The rules themselves are `orchester_core`'s constructors.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
};

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
