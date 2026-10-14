//! The local part of first project setup: turning what the user entered into a project and
//! the repository it lives in, and creating it with the GitHub repository detected in its folder
//! ([`create`]). The rules themselves are `orchester_core`'s constructors.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{GitHubHost, GitHubLogin},
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  repository::{Detection, DetectionState},
};
use orchester_store::StoreError;

use crate::{
  activity::{Activity, operations},
  background,
  messages::{self, Message},
  repository::adopt_in,
  workspace::{OpenProject, Workspace},
};

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

impl CreateFailure {
  pub fn from_store(new: &NewProject, error: &StoreError) -> Self {
    let place = match error {
      StoreError::NotAGitRepository(_) | StoreError::AlreadyInitialized(_) | StoreError::UnrepresentablePath(_) => {
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

/// How far the setup form is from Create: a folder must be chosen, and it must be a git repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
  NoFolder,
  /// Detection found no git repository in the chosen folder (01.C9).
  NotARepository,
  /// Create may be pressed; a name the rules reject is said under the name field then.
  Ready,
}

/// Where the form stands with `folder_chosen` and the folder's finished detection (`None` while it runs or
/// before it ran, which never holds Create back).
pub fn readiness(folder_chosen: bool, detection: Option<&DetectionState>) -> Readiness {
  match (folder_chosen, detection) {
    (false, _) => Readiness::NoFolder,
    (true, Some(state)) if state.blocks_project() => Readiness::NotARepository,
    (true, _) => Readiness::Ready,
  }
}

/// The account a project being set up expects: the one it expects already, else the account `gh` acted as
/// when `detection` ran. *Use this account* replaces it with the account `gh` acts as now.
pub fn expected_account(expected: Option<&GitHubLogin>, detection: &Detection) -> Option<GitHubLogin> {
  expected
    .or_else(|| detection.github().and_then(|found| found.account()))
    .cloned()
}

/// The name of the GitHub connection that setup adds for `host`.
pub fn github_connection_name(host: &GitHubHost) -> Message {
  Message::new("appMcp.githubConnectionName").with("host", host.to_string())
}

/// What Create makes: the project, and the GitHub repository detected in its folder, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creation {
  pub new: NewProject,
  pub link: Option<GitHubLink>,
}

/// The GitHub repository a detection found, which Create links: the account the project expects, and the
/// name of the connection it adds for the repository's host ([`github_connection_name`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubLink {
  pub detection: Detection,
  pub expected: Option<GitHubLogin>,
  pub connection_name: String,
}

/// Creates the project in its folder and adds it to the workspace, then records the GitHub repository
/// detected in the folder, each as an operation of `activity`. A link that fails is reported there only:
/// the project exists and opens either way, and can be linked from Project settings.
pub async fn create(
  workspace: &Workspace,
  activity: &Activity,
  creation: Creation,
) -> Result<OpenProject, CreateFailure> {
  let Creation { new, link: github } = creation;
  let attempted = new.clone();
  let running = activity.start(operations::project_create(new.project.name().as_str(), &new.repo_root));
  let workspace = workspace.clone();
  let created = background::run(move || workspace.create(new))
    .await
    .map_err(|message| CreateFailure::new(&attempted, message, ErrorPlace::Form))
    .and_then(|created| {
      created.map_err(|error| {
        tracing::error!(%error, "cannot create the project");
        CreateFailure::from_store(&attempted, &error)
      })
    });
  match &created {
    Ok(_) => running.finish(),
    Err(failure) => running.fail(failure.message.clone()),
  }
  if let (Ok(project), Some(github)) = (&created, github) {
    link(activity, project, github).await;
  }
  created
}

/// Records the GitHub connection and link of a created project; what failed is in `activity`.
async fn link(activity: &Activity, project: &OpenProject, github: GitHubLink) {
  let GitHubLink {
    detection,
    expected,
    connection_name: name,
  } = github;
  let Some(found) = detection.github() else {
    return;
  };
  let repository = format!("{}/{}", found.repo().owner(), found.repo().name());
  let spec = operations::repository_link(project.project.name().as_str(), &project.repo_root, &repository);
  let repo_root = project.repo_root.clone();
  let work = async move {
    background::run(move || adopt_in(&repo_root, &detection, expected.as_ref(), &name))
      .await?
      .map_err(|error| {
        tracing::error!(%error, "cannot record the detected GitHub repository");
        error.message()
      })
  };
  // The outcome is in the activity; the created project does not depend on it.
  if let Err(reason) = activity.track(spec, work).await {
    tracing::warn!(?reason, "the created project is not linked to its GitHub repository");
  }
}
