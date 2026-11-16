//! The tasks a pipeline's archive touches, for the Pipelines hub (`Pipelines · v2` P11 `nrTt0`, Archived): the tasks
//! that name the pipeline, which the archive confirmation lists because the store refuses their runs once it is
//! archived (`orchester_store`'s `UsesArchived`) until they name another pipeline or it is restored; and the tasks of
//! the runs that archiving stopped, which the archived pipeline's panel says stay stopped.

use std::path::Path;

use orchester_core::{id::ResourceId, store::TaskStore};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  agents::local_hub::{StoppedRun, stopped},
  background,
  messages::{self, Message},
};

/// A task that names a pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskUse {
  /// The task's key, *KON-9*.
  pub key: String,
  pub title: String,
}

/// The tasks of the project at `repo_root` that name `pipeline`, by their number.
pub async fn tasks_using(repo_root: &Path, pipeline: ResourceId) -> Result<Vec<TaskUse>, Message> {
  let root = repo_root.to_owned();
  background::run(move || tasks_using_in(&root, pipeline).map_err(|error| messages::store_error(&error))).await?
}

fn tasks_using_in(repo_root: &Path, pipeline: ResourceId) -> Result<Vec<TaskUse>, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  Ok(
    db.tasks()?
      .iter()
      .filter(|task| task.details().pipeline == Some(pipeline))
      .map(|task| TaskUse {
        key: task.key().to_string(),
        title: task.details().title.to_string(),
      })
      .collect(),
  )
}

/// The tasks of the runs the archive of `pipeline` stopped, oldest first.
pub async fn stopped_by_archive(repo_root: &Path, pipeline: ResourceId) -> Result<Vec<StoppedRun>, Message> {
  let root = repo_root.to_owned();
  background::run(move || {
    let db = ProjectDb::open(&root)?;
    stopped(&db, &db.archive_stops_of_resource(pipeline)?)
  })
  .await?
  .map_err(|error| messages::store_error(&error))
}
