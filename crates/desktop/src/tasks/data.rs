//! Reading and saving tasks through the project database. Every function blocks on SQLite;
//! the UI calls them off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  resource::{Pipeline, ResourceKind, VersionNumber},
  store::{ResourceStore, TaskStore},
  task::{Task, TaskDetails, TaskStatus},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

/// A pipeline a task can use: one with a published version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
  /// The latest published version, which new runs use.
  pub version: VersionNumber,
  pub steps: Vec<Arc<str>>,
}

/// The project's tasks and the pipelines they can use, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectTasks {
  pub tasks: Arc<Vec<Task>>,
  pub pipelines: Arc<Vec<PipelineChoice>>,
}

impl lurq::app::component::DevtoolsInspectable for ProjectTasks {}

impl ProjectTasks {
  pub fn load(repo_root: &Path) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    Ok(Self {
      tasks: Arc::new(db.tasks()?),
      pipelines: Arc::new(published_pipelines(&db)?),
    })
  }
}

/// Pipelines with a published version, ordered by slug. Drafts that were never published
/// cannot be run, so they are not offered.
pub fn published_pipelines<S: ResourceStore>(store: &S) -> Result<Vec<PipelineChoice>, S::Error> {
  let mut choices = Vec::new();
  for resource in store.resources(ResourceKind::Pipeline)? {
    let Some(version) = store.latest_version::<Pipeline>(resource.id())? else {
      continue;
    };
    choices.push(PipelineChoice {
      id: resource.id(),
      name: resource.name().as_str().into(),
      version: version.number(),
      steps: version
        .body()
        .steps()
        .iter()
        .map(|step| step.name.as_str().into())
        .collect(),
    });
  }
  Ok(choices)
}

/// Saves `details` as one draft task under `batch`, the key the form generated for this
/// save. Retrying with the same key never creates a second task: if an earlier attempt was
/// stored after all, that task is returned, with the details of this attempt if they were
/// edited since and the task is still a draft.
pub fn save_draft<S: TaskStore>(
  store: &mut S,
  batch: IdempotencyKey,
  details: TaskDetails,
  now: Timestamp,
) -> Result<Task, S::Error> {
  let task = store
    .create_tasks(batch, vec![details.clone()], now)?
    .into_iter()
    .next()
    .expect("create_tasks returns one task for each of the one details given");
  if task.details() == &details || task.status() != TaskStatus::Draft {
    return Ok(task);
  }
  let edited = Task::from_stored(task.id(), task.key().clone(), details, task.status(), task.created_at());
  store.save_task(&edited)?;
  Ok(edited)
}

/// [`save_draft`] in the project database of `repo_root`.
pub fn save_draft_in(repo_root: &Path, batch: IdempotencyKey, details: TaskDetails) -> Result<Task, StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  save_draft(&mut db, batch, details, Timestamp::now())
}
