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
  pub key: &'a str,
  pub repository: Option<&'a Path>,
}

/// A validated project, ready to be created in `repo_root`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewProject {
  pub project: Project,
  pub repo_root: PathBuf,
}

/// Per-field problems, each as a message to show under its field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DraftErrors {
  pub name: Option<Message>,
  pub key: Option<Message>,
  pub repository: Option<Message>,
}

impl lurq::app::component::DevtoolsInspectable for DraftErrors {}

impl ProjectDraft<'_> {
  pub fn validate(&self) -> Result<NewProject, DraftErrors> {
    let name = Name::new(self.name);
    let key = ProjectKey::new(self.key);
    match (name, key, self.repository) {
      (Ok(name), Ok(key), Some(repository)) => Ok(NewProject {
        project: Project::new(ProjectId::generate(), key, name),
        repo_root: repository.to_path_buf(),
      }),
      (name, key, repository) => Err(DraftErrors {
        name: name.err().as_ref().map(messages::name_error),
        key: key.err().as_ref().map(messages::project_key_error),
        repository: repository.is_none().then(messages::repository_required),
      }),
    }
  }
}
