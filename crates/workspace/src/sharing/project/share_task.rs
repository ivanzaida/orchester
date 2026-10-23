//! *Share task* (`docs/architecture/server.md`, "Unticked tasks: archived as *Not shared*"; design
//! 10.24, 10.26 A): a task kept *Not shared* when the project joined goes to the server through
//! the same import as joining, with the next number, its id and its key as its former key, and its
//! tags by the names it kept. Its runs, conversation and memory stay attached by its id.

use std::path::Path;

use orchester_api::{ErrorCode, TasksImport};
use orchester_core::{
  id::{IdempotencyKey, TaskId},
  project::TaskKey,
  store::TaskStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{ProjectServer, organization::tag, share, store::RemoteTaskStore};
use crate::{background, sharing::SharingError};

impl ProjectServer {
  /// The key *Share task* would give a *Not shared* task now: the server project's next number, which the server may
  /// give another task first.
  pub async fn next_key(&self) -> Result<TaskKey, SharingError> {
    let project = self
      .call(|client, token, project| async move { client.project(&token, project).await })
      .await?;
    let number = u32::try_from(project.next_number)
      .ok()
      .and_then(std::num::NonZeroU32::new)
      .ok_or(SharingError::InvalidAnswer("next number"))?;
    Ok(TaskKey::new(self.link.key.clone(), number))
  }

  /// Uploads the *Not shared* `task` under `key` and caches it as the server's; returns its key.
  pub async fn share_kept(&self, task: TaskId, key: IdempotencyKey) -> Result<TaskKey, SharingError> {
    self.writable()?;
    let root = self.repo_root.clone();
    let import = background::run(move || kept_upload(&root, task, key)).await??;
    let created = match self
      .call(|client, token, project| async move { client.import_tasks(&token, project, &import).await })
      .await
    {
      Ok(created) => created.tasks,
      // A share whose answer was lost made the task the server's already: it is cached as the server holds it.
      Err(error) if error.code() == Some(ErrorCode::IdTaken) => {
        let held: Vec<_> = self
          .call(|client, token, project| async move { client.tasks(&token, project).await })
          .await?
          .tasks
          .into_iter()
          .filter(|held| held.id == task)
          .collect();
        if held.is_empty() {
          return Err(error);
        }
        held
      }
      Err(error) => return Err(error),
    };
    let tags = self
      .call(|client, token, project| async move { client.tags(&token, project).await })
      .await?;
    let number = created
      .first()
      .and_then(|task| std::num::NonZeroU32::new(task.number))
      .ok_or(SharingError::InvalidAnswer("imported task"))?;
    let server = self.clone();
    background::run(move || -> Result<(), SharingError> {
      let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
      let mut store = RemoteTaskStore::new(ProjectDb::open(server.repo_root())?, server)?;
      store.db.cache_shared_tags(&tags)?;
      store.cache(&created, None)
    })
    .await??;
    self.servers.shared_changed();
    Ok(TaskKey::new(self.link.key.clone(), number))
  }
}

/// What a *Not shared* task's page says of it (`R4elt`): when it was left out, the server project's key, and the
/// shared task whose title looks like its own, as a hint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptPreview {
  pub joined_at: orchester_core::time::Timestamp,
  pub key: orchester_core::project::ProjectKey,
  pub address: Option<String>,
  pub similar: Option<super::SimilarTask>,
}

/// [`KeptPreview`] of the *Not shared* `task` of the project at `root`. Blocking.
pub fn kept_preview(root: &Path, task: TaskId) -> Result<KeptPreview, StoreError> {
  let db = ProjectDb::open(root)?;
  let link = db.sharing()?.ok_or(StoreError::NotShared)?;
  let kept = db.task(task)?.ok_or(StoreError::TaskNotFound(task))?;
  let shared = db.tasks()?;
  let similar = orchester_core::sharing::similar_titles(
    kept.details().title.as_str(),
    shared.iter().map(|task| task.details().title.as_str()),
  )
  .first()
  .and_then(|index| shared.get(*index))
  .map(|found| -> Result<_, StoreError> {
    let created_by = db
      .shared_task_info(found.id())?
      .and_then(|info| info.created_by)
      .unwrap_or_default();
    Ok(super::SimilarTask {
      key: found.key().to_string(),
      title: found.details().title.as_str().to_owned(),
      created_by,
    })
  })
  .transpose()?;
  Ok(KeptPreview {
    joined_at: link.joined_at,
    key: link.key,
    address: link.address,
    similar,
  })
}

/// The import of the kept `task`, with the tag names it kept and its module.
fn kept_upload(root: &Path, task: TaskId, key: IdempotencyKey) -> Result<TasksImport, StoreError> {
  let db = ProjectDb::open(root)?;
  let kept = db.task(task)?.ok_or(StoreError::TaskNotFound(task))?;
  let names = db
    .shared_task_info(task)?
    .and_then(|info| info.kept_tags)
    .ok_or(StoreError::TaskNotFound(task))?;
  let mut upload = share::upload_task(&db, &kept, &std::collections::HashMap::new(), share::UploadAs::Import)?;
  upload.tag_names = names;
  let module = kept.module().map(|id| db.module(id)).transpose()?.flatten();
  Ok(TasksImport {
    idempotency_key: key,
    tasks: vec![upload],
    modules: module.iter().map(super::convert::upload_module).collect(),
  })
}
