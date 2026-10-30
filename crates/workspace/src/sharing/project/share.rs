//! *Share* (`docs/architecture/server.md`, "Migration", rows 12 and 49; design 10.11, 10.12): an admin made the
//! server project, empty, in the server admin; the first checkout linked to it uploads every task with its number,
//! its organization (tags by name), its module and its pipeline's slug, with every module of the project, in one
//! request, and its `origin` becomes the project's
//! remote hint when it has none. Tasks with a run that has not ended are claimed for
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

use orchester_api::{Organization, ProjectLink, SharedProject, UploadTask, UserRef};
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
  sharing::{ProjectAddress, ServerEntry, Servers, SharingError},
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
  /// The server project has tasks already: linking this checkout to it is a Join, which gives the local tasks new
  /// numbers.
  #[error("the server project has tasks already")]
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
  let (tasks, next, claimed) = upload_keys(&ProjectDb::open(repo_root)?)?;
  Ok(SharePlan {
    identity,
    tasks,
    next,
    claimed,
  })
}

/// What an upload that keeps numbers takes from `db`: its tasks' keys by number, the key the server gives the next
/// new task, and the keys of the tasks with a run that has not ended, which are claimed for the person.
pub(super) fn upload_keys(db: &ProjectDb) -> Result<(Vec<TaskKey>, TaskKey, Vec<TaskKey>), StoreError> {
  let mut tasks = db.tasks()?;
  tasks.sort_by_key(|task| task.key().number());
  let mut claimed = Vec::new();
  for task in &tasks {
    if db.active_task_run(task.id())?.is_some() {
      claimed.push(task.key().clone());
    }
  }
  let highest = tasks.iter().map(|task| task.key().number().get()).max().unwrap_or(0);
  let next = NonZeroU32::MIN.saturating_add(highest);
  Ok((
    tasks.iter().map(|task| task.key().clone()).collect(),
    TaskKey::new(db.project().key().clone(), next),
    claimed,
  ))
}

/// What linking a local project to a server project as its first checkout needs.
pub struct LinkFirst {
  /// The project's slug on the server (the last segment of its address).
  pub slug: String,
  /// The checkout's `origin`, for the project's remote hint; `None` without a network one.
  pub remote: Option<RepositoryIdentity>,
  /// The upload's idempotency key, made when the confirmation opened: a retry links once.
  pub key: IdempotencyKey,
  /// Whether to write the discovery file naming the project's address.
  pub write_discovery: bool,
}

impl Servers {
  /// Links the project at `repo_root` to the empty server project at `request.slug` on `entry` as its first checkout:
  /// every task is uploaded with its number.
  pub async fn link_first(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    request: LinkFirst,
  ) -> Result<Shared, ShareError> {
    let project = self.find_project(entry, &request.slug).await?;
    self
      .link_upload(
        entry,
        repo_root,
        project,
        (request.remote, request.key),
        request.write_discovery,
      )
      .await
  }

  /// Shares the project at `repo_root` on `entry` into the person's empty project there that was set up for
  /// `identity` (its remote hint), and writes the discovery file when `write_discovery` asks for it. Since API 3 a
  /// project is found by its address ([`Self::link_first`]); this finds it by the hint.
  pub async fn share(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    identity: RepositoryIdentity,
    key: IdempotencyKey,
    write_discovery: bool,
  ) -> Result<Shared, ShareError> {
    let project = self
      .project_by_hint(entry, &identity)
      .await?
      .ok_or(SharingError::NoProject)?;
    self
      .link_upload(entry, repo_root, project, (Some(identity), key), write_discovery)
      .await
  }

  pub(super) async fn link_upload(
    &self,
    entry: &ServerEntry,
    repo_root: &Path,
    project: SharedProject,
    (remote, key): (Option<RepositoryIdentity>, IdempotencyKey),
    write_discovery: bool,
  ) -> Result<Shared, ShareError> {
    // Every first link goes through here: a checkout linked already uploads nothing into another project.
    super::join::still_local(repo_root).await?;
    let address = ProjectAddress::reported(&project.address, &entry.address)?.to_text();
    let id = project.id;
    let on_server = self
      .with_token(entry, |client, token| async move { client.tasks(&token, id).await })
      .await?;
    if !on_server.tasks.is_empty() {
      return Err(ShareError::Taken {
        project: id,
        member: true,
        owners: Vec::<UserRef>::new(),
      });
    }
    let _in_progress = InProgress::start(self, repo_root);
    self.keep_with_server(entry, repo_root).await?;
    let root = repo_root.to_owned();
    let (link, uploaded) = background::run(move || upload(&root, remote.as_ref(), key))
      .await
      .map_err(SharingError::from)??;
    let linked = self
      .with_token(entry, |client, token| async move {
        client.link_project(&token, id, &link).await
      })
      .await?;
    let tags = self
      .with_token(entry, |client, token| async move { client.tags(&token, id).await })
      .await?;
    let link = SharingLink {
      server: entry.stored.id,
      project: id,
      key: orchester_core::project::ProjectKey::new(&project.key)
        .map_err(|_| SharingError::InvalidAnswer("project key"))?,
      role: project.role,
      joined_at: Timestamp::now(),
      cursor: linked.cursor,
      last_synced_at: Timestamp::now(),
      removed_at: None,
      address: Some(address.clone()),
    };
    let (root, stored) = (repo_root.to_owned(), link.clone());
    let servers = self.clone();
    let discovery = background::run(move || -> Result<_, SharingError> {
      let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
      let tasks = linked
        .tasks
        .iter()
        .map(convert::cached)
        .collect::<Result<Vec<_>, _>>()?;
      let modules = linked
        .modules
        .iter()
        .map(convert::cached_module)
        .collect::<Result<Vec<_>, _>>()?;
      let mut db = ProjectDb::open(&root)?;
      db.adopt_shared_modules(&modules)?;
      let changed = db.start_sharing(&stored, &tags, (&tasks, &uploaded))?;
      if let Err(error) = reapply(&servers, &root, changed) {
        tracing::error!(%error, "a run's change made while the project was shared was not sent");
      }
      Ok(write_discovery.then(|| discovery::write(&root, &address)))
    })
    .await
    .map_err(SharingError::from)??;
    self.shared_changed();
    Ok(Shared { link, discovery })
  }
}

/// The project at `repo_root` takes no change from this computer's screens while it is shared or
/// joins a server project. A link holds it from its check of the reviewed tasks on, around the share or join it runs,
/// which then only finds it held: the outermost one ends it.
pub(super) struct InProgress {
  servers: Servers,
  root: PathBuf,
  outermost: bool,
}

impl InProgress {
  pub(super) fn start(servers: &Servers, root: &Path) -> Self {
    let outermost = servers.inner.sharing_now.lock().insert(root.to_owned());
    Self {
      servers: servers.clone(),
      root: root.to_owned(),
      outermost,
    }
  }
}

impl Drop for InProgress {
  fn drop(&mut self) {
    if self.outermost {
      self.servers.inner.sharing_now.lock().remove(&self.root);
    }
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

/// The upload of every task of the project at `repo_root` with its number, with the revision of each.
fn upload(
  repo_root: &Path,
  remote: Option<&RepositoryIdentity>,
  key: IdempotencyKey,
) -> Result<(ProjectLink, HashMap<TaskId, TaskRevision>), StoreError> {
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
  let link = ProjectLink {
    idempotency_key: key,
    tasks,
    remote: remote.map(|identity| identity.as_str().to_owned()),
    modules: db.modules()?.iter().map(convert::upload_module).collect(),
  };
  Ok((link, revisions))
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
    module: task.module(),
    status: task.status(),
    created_at: task.created_at().unix_millis(),
    claim: db.active_task_run(task.id())?.is_some(),
    has_run,
  })
}
