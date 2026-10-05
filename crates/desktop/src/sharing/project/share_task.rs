//! *Share task* (`docs/architecture/server.md`, "Unticked tasks: archived as *Not shared*"; design
//! 10.24, 10.26 A): a task kept *Not shared* when the project joined goes to the server through
//! the same import as joining, with the next number, its id and its key as its former key, and its
//! tags by the names it kept. Its runs, conversation and memory stay attached by its id.

use std::path::Path;

use orchester_api::TasksImport;
use orchester_core::{
  id::{IdempotencyKey, TaskId},
  project::TaskKey,
  store::TaskStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{ProjectServer, organization::tag, share, store::RemoteTaskStore};
use crate::{background, sharing::SharingError};

impl ProjectServer {
  /// Uploads the *Not shared* `task` under `key` and caches it as the server's; returns its key.
  pub async fn share_kept(&self, task: TaskId, key: IdempotencyKey) -> Result<TaskKey, SharingError> {
    self.writable()?;
    let root = self.repo_root.clone();
    let import = background::run(move || kept_upload(&root, task, key)).await??;
    let created = self
      .call(|client, token, project| async move { client.import_tasks(&token, project, &import).await })
      .await?;
    let tags = self
      .call(|client, token, project| async move { client.tags(&token, project).await })
      .await?;
    let number = created
      .tasks
      .first()
      .and_then(|task| std::num::NonZeroU32::new(task.number))
      .ok_or(SharingError::InvalidAnswer("imported task"))?;
    let server = self.clone();
    background::run(move || -> Result<(), SharingError> {
      let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
      let mut store = RemoteTaskStore::new(ProjectDb::open(server.repo_root())?, server)?;
      store.db.cache_shared_tags(&tags)?;
      store.cache(&created.tasks, None)
    })
    .await??;
    self.servers.shared_changed();
    Ok(TaskKey::new(self.link.key.clone(), number))
  }
}

/// The import of the kept `task`, with the tag names it kept.
fn kept_upload(root: &Path, task: TaskId, key: IdempotencyKey) -> Result<TasksImport, StoreError> {
  let db = ProjectDb::open(root)?;
  let kept = db.task(task)?.ok_or(StoreError::TaskNotFound(task))?;
  let names = db
    .shared_task_info(task)?
    .and_then(|info| info.kept_tags)
    .ok_or(StoreError::TaskNotFound(task))?;
  let mut upload = share::upload_task(&db, &kept, &std::collections::HashMap::new(), share::UploadAs::Import)?;
  upload.tag_names = names;
  Ok(TasksImport {
    idempotency_key: key,
    tasks: vec![upload],
  })
}
