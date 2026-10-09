//! *Share* (`docs/architecture/server.md`, "Migration"; design 10.11, 10.12): the server project is
//! made from the local one and every task is uploaded with its number, its organization (tags by
//! name) and its pipeline's slug, in one request. Tasks with a run that has not ended are claimed for
//! the person. The cache then holds what the server confirmed, and the project shares its task list
//! from here on. The request is idempotent by the key the confirmation was opened with, so a retry
//! after a lost answer shares once. While it is under way the project's tasks take no change from
//! this computer's screens; a run's change made meanwhile is sent once the project is linked, so
//! the server's copy of the upload does not replace it.

use std::{
  collections::HashMap,
  num::NonZeroU32,
  path::{Path, PathBuf},
};

use orchester_api::{ErrorCode, Organization, ProjectCreate, SharingErrorDetails, UploadTask, UserRef};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, TaskId},
  project::TaskKey,
  repository::{RemoteName, RepositoryIdentity, RepositoryIdentityError, read_remotes},
  sharing::TaskRevision,
  store::{RunStore, TaskStore},
  task::{Task, TaskUpdate, organization::ChangeActor},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, SharingLink, StoreError};

use super::{ProjectServer, convert, discovery, organization::tag, store::RemoteTaskStore};
use crate::{
  background,
  sharing::{ServerEntry, Servers, SharingError},
};

/// What sharing the project would do, as the confirmation says it (10.12).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharePlan {
  pub identity: RepositoryIdentity,
  /// The tasks uploaded, ordered by number: their keys are kept.
  pub tasks: Vec<TaskKey>,
  /// The key the server gives the next new task.
  pub next: TaskKey,
  /// Tasks with a run that has not ended: they are claimed for the person.
  pub claimed: Vec<TaskKey>,
}

/// The project shared, and whether the discovery file was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shared {
  pub link: SharingLink,
  /// `None` when the person did not ask for it; otherwise whether writing it worked.
  pub discovery: Option<Result<(), discovery::DiscoveryError>>,
}

/// Why the project was not shared.
#[derive(Debug, thiserror::Error)]
pub enum ShareError {
  #[error(transparent)]
  Sharing(#[from] SharingError),
  /// The repository is shared on the server already (`409 remote_taken`): joining it is a Join,
  /// never a second project.
  #[error("the repository is shared on the server already")]
  Taken {
    project: ProjectId,
    member: bool,
    owners: Vec<UserRef>,
  },
}

impl From<StoreError> for ShareError {
  fn from(error: StoreError) -> Self {
    Self::Sharing(error.into())
  }
}

/// The identity of the repository at `repo_root`, from its `origin` remote: what a server knows a
/// project by. Blocking.
pub fn repository_identity(programs: &Programs, repo_root: &Path) -> Result<RepositoryIdentity, IdentityProblem> {
  let remotes =
    read_remotes(&programs.runner(), Path::new("git"), repo_root).map_err(|_| IdentityProblem::Unreadable)?;
  let origin = RemoteName::origin();
  let url = remotes
    .iter()
    .find(|remote| remote.name() == &origin)
    .map(|remote| remote.url().to_owned())
    .unwrap_or_default();
  RepositoryIdentity::from_remote_url(&url).map_err(|error| match error {
    RepositoryIdentityError::MissingOrigin => IdentityProblem::NoOrigin,
    // The remote's address as git lists it, credentials already removed when it was read.
    RepositoryIdentityError::LocalOrigin => IdentityProblem::LocalOrigin { origin: Some(url) },
    RepositoryIdentityError::Invalid => IdentityProblem::LocalOrigin { origin: None },
  })
}

/// Why the repository cannot be shared (10.16).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityProblem {
  NoOrigin,
  /// `origin` is a folder on this computer (`origin` names it), or not a network repository's
  /// address.
  LocalOrigin {
    origin: Option<String>,
  },
  /// git could not list the remotes.
  Unreadable,
}

/// What sharing the project at `repo_root` as `identity` would do. Blocking.
pub fn plan(repo_root: &Path, identity: RepositoryIdentity) -> Result<SharePlan, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  let tasks = db.tasks()?;
  let mut claimed = Vec::new();
  for task in &tasks {
    if db.active_task_run(task.id())?.is_some() {
      claimed.push(task.key().clone());
    }
  }
  let highest = tasks.iter().map(|task| task.key().number().get()).max().unwrap_or(0);
  let next = NonZeroU32::MIN.saturating_add(highest);
  Ok(SharePlan {
    identity,
    next: TaskKey::new(db.project().key().clone(), next),
    tasks: tasks.iter().map(|task| task.key().clone()).collect(),
    claimed,
  })
}

impl Servers {
  /// Shares the project at `repo_root` on `entry` as `identity` under `key`, and writes the
  /// discovery file when `discovery` asks for it.
  pub async fn share(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    identity: RepositoryIdentity,
    key: IdempotencyKey,
    write_discovery: bool,
  ) -> Result<Shared, ShareError> {
    let _in_progress = InProgress::start(self, repo_root);
    let root = repo_root.to_owned();
    let (create, uploaded) = background::run(move || upload(&root, &identity, key))
      .await
      .map_err(SharingError::from)??;
    let answer = self
      .with_token(entry, |client, token| async move {
        client.create_project(&token, &create).await
      })
      .await;
    let created = match answer {
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::RemoteTaken) => {
        return Err(taken(error.details()));
      }
      answer => answer?,
    };
    let project = created.project.id;
    let tags = self
      .with_token(entry, |client, token| async move { client.tags(&token, project).await })
      .await?;
    let link = SharingLink {
      server: entry.stored.id,
      project,
      key: orchester_core::project::ProjectKey::new(&created.project.key)
        .map_err(|_| SharingError::InvalidAnswer("project key"))?,
      role: created.project.role,
      joined_at: Timestamp::now(),
      cursor: created.cursor,
      last_synced_at: Timestamp::now(),
      removed_at: None,
    };
    let (root, stored, url) = (repo_root.to_owned(), link.clone(), entry.address.as_str().to_owned());
    let servers = self.clone();
    let discovery = background::run(move || -> Result<_, SharingError> {
      let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
      let tasks = created
        .tasks
        .iter()
        .map(convert::cached)
        .collect::<Result<Vec<_>, _>>()?;
      let changed = ProjectDb::open(&root)?.start_sharing(&stored, &tags, (&tasks, &uploaded))?;
      if let Err(error) = reapply(&servers, &root, changed) {
        tracing::error!(%error, "a run's change made while the project was shared was not sent");
      }
      Ok(write_discovery.then(|| discovery::write(&root, &url)))
    })
    .await
    .map_err(SharingError::from)??;
    self.shared_changed();
    Ok(Shared { link, discovery })
  }
}

fn taken(details: Option<&SharingErrorDetails>) -> ShareError {
  match details {
    Some(SharingErrorDetails::Remote {
      project,
      member,
      owners,
    }) => ShareError::Taken {
      project: *project,
      member: *member,
      owners: owners.clone(),
    },
    _ => ShareError::Sharing(SharingError::InvalidAnswer("remote taken")),
  }
}

/// The project at `repo_root` takes no change from this computer's screens while it is shared or
/// joins a server project.
pub(super) struct InProgress {
  servers: Servers,
  root: PathBuf,
}

impl InProgress {
  pub(super) fn start(servers: &Servers, root: &Path) -> Self {
    servers.inner.sharing_now.lock().insert(root.to_owned());
    Self {
      servers: servers.clone(),
      root: root.to_owned(),
    }
  }
}

impl Drop for InProgress {
  fn drop(&mut self) {
    self.servers.inner.sharing_now.lock().remove(&self.root);
  }
}

/// Sends `changed`, as this computer has them, on top of the uploaded copies the cache now holds
/// (Share and Join).
pub(super) fn reapply(servers: &Servers, root: &Path, changed: Vec<Task>) -> Result<(), SharingError> {
  if changed.is_empty() {
    return Ok(());
  }
  let server = ProjectServer::open(servers, root)?.ok_or(StoreError::NotShared)?;
  let mut store = RemoteTaskStore::new(ProjectDb::open(root)?, server)?;
  for task in changed {
    let cached = store.cached(task.id())?;
    if let TaskUpdate::Conflict { .. } = store.save_task(&task.clone().with_revision(cached.revision()))? {
      tracing::warn!(task = %task.id(), "the task changed on the server before its run's change was sent");
      continue;
    }
    store.change_organization(
      task.id(),
      (cached.organization(), task.organization()),
      ChangeActor::Orchester,
    )?;
  }
  Ok(())
}

/// The request that creates the server project from the project at `repo_root`, with the revision
/// of each task it uploads.
fn upload(
  repo_root: &Path,
  identity: &RepositoryIdentity,
  key: IdempotencyKey,
) -> Result<(ProjectCreate, HashMap<TaskId, TaskRevision>), StoreError> {
  let db = ProjectDb::open(repo_root)?;
  let names: HashMap<_, _> = db
    .project_tags()?
    .into_iter()
    .map(|tag| (tag.id(), tag.name().as_str().to_owned()))
    .collect();
  let read = db.tasks()?;
  let revisions = read.iter().map(|task| (task.id(), task.revision())).collect();
  let tasks = read
    .iter()
    .map(|task| upload_task(&db, task, &names, UploadAs::Share))
    .collect::<Result<Vec<_>, _>>()?;
  let create = ProjectCreate {
    idempotency_key: key,
    name: db.project().name().as_str().to_owned(),
    key: db.project().key().as_str().to_owned(),
    remote: identity.as_str().to_owned(),
    tasks,
  };
  Ok((create, revisions))
}

/// How a task goes to the server: with its number when the project is shared, or for a new number
/// with its key as its former key when it is imported (*Join*, *Share task*).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum UploadAs {
  Share,
  Import,
}

pub(super) fn upload_task(
  db: &ProjectDb,
  task: &Task,
  names: &HashMap<orchester_core::id::TagId, String>,
  this: UploadAs,
) -> Result<UploadTask, StoreError> {
  let details = task.details();
  let organization = Organization::from_core(task.organization());
  let has_run = !db
    .runs_of_task(task.id())
    .map_err(crate::tasks::detail::run_store_error)?
    .is_empty();
  Ok(UploadTask {
    id: task.id(),
    number: (this == UploadAs::Share).then(|| task.key().number().get()),
    former_key: (this == UploadAs::Import).then(|| task.key().to_string()),
    title: details.title.as_str().to_owned(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: convert::pipeline_slug(db, details.pipeline)?,
    limits: convert::shared_limits(details.limits),
    workflow: organization.workflow,
    priority: organization.priority,
    tag_names: organization
      .tags
      .iter()
      .filter_map(|id| names.get(id).cloned())
      .collect(),
    blocking: organization.blocking,
    blocked_reason: organization.blocked_reason,
    status: task.status(),
    created_at: task.created_at().unix_millis(),
    claim: db.active_task_run(task.id())?.is_some(),
    has_run,
  })
}
