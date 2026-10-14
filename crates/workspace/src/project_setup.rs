//! The local part of first project setup: turning what the user entered into a project and
//! the repository it lives in, and creating it with the GitHub repository detected in its folder
//! ([`create`]). The rules themselves are `orchester_core`'s constructors.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{GitHubHost, GitHubLogin},
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  repository::{Detection, DetectionState, RepositoryLink},
};
use orchester_store::{RegisteredProject, StoreError};

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

/// Why creating the project failed, as automation names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateCause {
  NotAGitRepository,
  /// The folder keeps an Orchester project already.
  AlreadyInitialized,
  /// The list holds a project in the folder already, e.g. one whose data is gone.
  AlreadyListed,
  /// The project data could not be written, or the work stopped.
  ProjectData,
}

impl CreateCause {
  pub const fn code(self) -> &'static str {
    match self {
      Self::NotAGitRepository => "not_a_git_repository",
      Self::AlreadyInitialized => "already_initialized",
      Self::AlreadyListed => "already_listed",
      Self::ProjectData => "project_data",
    }
  }
}

/// Creating the project failed. It stays relevant only while the name and folder it was
/// raised for are unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateFailure {
  pub message: Message,
  pub place: ErrorPlace,
  pub cause: CreateCause,
  name: String,
  repo_root: PathBuf,
}

impl CreateFailure {
  pub fn from_store(new: &NewProject, error: &StoreError) -> Self {
    let (place, cause) = match error {
      StoreError::NotAGitRepository(_) => (ErrorPlace::Repository, CreateCause::NotAGitRepository),
      StoreError::AlreadyInitialized(_) => (ErrorPlace::Repository, CreateCause::AlreadyInitialized),
      StoreError::UnrepresentablePath(_) => (ErrorPlace::Repository, CreateCause::ProjectData),
      _ => (ErrorPlace::Form, CreateCause::ProjectData),
    };
    Self::new(new, messages::store_error(error), place).caused_by(cause)
  }

  pub fn new(new: &NewProject, message: Message, place: ErrorPlace) -> Self {
    Self {
      message,
      place,
      cause: CreateCause::ProjectData,
      name: new.project.name().as_str().to_owned(),
      repo_root: new.repo_root.clone(),
    }
  }

  #[must_use]
  pub const fn caused_by(mut self, cause: CreateCause) -> Self {
    self.cause = cause;
    self
  }

  /// The list holds `listed` in the folder of `new` already: its entry has to go first.
  fn listed(new: &NewProject, listed: &RegisteredProject) -> Self {
    let message = Message::new("errors.alreadyListed").with("name", listed.name.to_string());
    Self::new(new, message, ErrorPlace::Repository).caused_by(CreateCause::AlreadyListed)
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

/// A detection and the folder it was made for: setup uses it only while that folder is the chosen one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderDetection {
  folder: PathBuf,
  detection: Detection,
}

impl FolderDetection {
  pub const fn new(folder: PathBuf, detection: Detection) -> Self {
    Self { folder, detection }
  }

  /// The detection, when it was made for `folder`.
  pub fn of(&self, folder: Option<&Path>) -> Option<&Detection> {
    (folder == Some(self.folder.as_path())).then_some(&self.detection)
  }

  pub fn folder(&self) -> &Path {
    &self.folder
  }

  pub const fn detection(&self) -> &Detection {
    &self.detection
  }
}

/// The account a project being set up expects, with the folder and GitHub host it was taken for: it applies
/// only to them, so another folder or host starts from the account `gh` acts as there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedAccount {
  login: GitHubLogin,
  folder: PathBuf,
  host: GitHubHost,
}

impl ExpectedAccount {
  /// The account `gh` acted as when it detected `folder`, for that folder and host (*Use this account*);
  /// `None` without one.
  pub fn acting(folder: &Path, detection: &Detection) -> Option<Self> {
    let found = detection.github()?;
    Some(Self {
      login: found.account()?.clone(),
      folder: folder.to_path_buf(),
      host: found.repo().host().clone(),
    })
  }

  pub const fn login(&self) -> &GitHubLogin {
    &self.login
  }

  /// Whether it was taken for `folder` and the GitHub host `detection` found there.
  pub fn applies_to(&self, folder: &Path, detection: &Detection) -> bool {
    self.folder == folder
      && detection
        .github()
        .is_some_and(|found| found.repo().host() == &self.host)
  }
}

/// The account the project expects after `detection` of `folder`: the one it expects already when that was
/// taken for the same folder and host, else the account `gh` acted as.
pub fn expected_account(
  current: Option<&ExpectedAccount>,
  folder: &Path,
  detection: &Detection,
) -> Option<ExpectedAccount> {
  current
    .filter(|current| current.applies_to(folder, detection))
    .cloned()
    .or_else(|| ExpectedAccount::acting(folder, detection))
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

/// A created project, and what became of its GitHub link: `None` when nothing was to be linked, else the
/// link (`None` when the account could not read the repository) or why recording it failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
  pub project: OpenProject,
  pub link: Option<Result<Option<RepositoryLink>, Message>>,
}

/// Creates the project in its folder and adds it to the workspace, then records the GitHub repository
/// detected in the folder, each as an operation of `activity`. A folder the list holds already is refused
/// before anything is written. A link that fails does not undo the project, which can be linked from
/// Project settings.
pub async fn create(workspace: &Workspace, activity: &Activity, creation: Creation) -> Result<Created, CreateFailure> {
  let Creation { new, link: github } = creation;
  let attempted = new.clone();
  let running = activity.start(operations::project_create(new.project.name().as_str(), &new.repo_root));
  let workspace = workspace.clone();
  let created = background::run(move || create_unlisted(&workspace, &new))
    .await
    .map_err(|message| CreateFailure::new(&attempted, message, ErrorPlace::Form))
    .and_then(|created| created);
  match &created {
    Ok(_) => running.finish(),
    Err(failure) => running.fail(failure.message.clone()),
  }
  let project = created?;
  let link = match github {
    Some(github) => link(activity, &project, github).await,
    None => None,
  };
  Ok(Created { project, link })
}

/// Creates `new` unless the list holds a project in its folder. Blocks on SQLite and the file system.
fn create_unlisted(workspace: &Workspace, new: &NewProject) -> Result<OpenProject, CreateFailure> {
  let store = |error: &StoreError| {
    tracing::error!(%error, "cannot create the project");
    CreateFailure::from_store(new, error)
  };
  let listed = workspace.projects().map_err(|error| store(&error))?;
  if let Some(listed) = listed
    .iter()
    .find(|listed| same_folder(&listed.repo_root, &new.repo_root))
  {
    return Err(CreateFailure::listed(new, listed));
  }
  workspace.create(new.clone()).map_err(|error| store(&error))
}

/// Whether `a` and `b` name one folder: the same path, or the same folder once both are resolved.
fn same_folder(a: &Path, b: &Path) -> bool {
  a == b
    || matches!(
      (std::fs::canonicalize(a), std::fs::canonicalize(b)),
      (Ok(a), Ok(b)) if a == b
    )
}

/// Records the GitHub connection and link of a created project, as an operation of `activity`; `None` when
/// the detection found no GitHub repository.
async fn link(
  activity: &Activity,
  project: &OpenProject,
  github: GitHubLink,
) -> Option<Result<Option<RepositoryLink>, Message>> {
  let GitHubLink {
    detection,
    expected,
    connection_name: name,
  } = github;
  let found = detection.github()?;
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
  // The created project does not depend on the outcome, which the activity lists too.
  Some(activity.track(spec, work).await)
}
