//! The board's moves that need no runner ([`super::Move`]): a workflow status changed by hand, a complete draft queued
//! on its pipeline's latest published version and set to Todo, and a queued task whose run never started taken back
//! to the drafts and to Backlog. A shared task changes on its server, as every change through the task writer does;
//! queuing it takes its run claim first. Every function blocks on SQLite (and the server); the UI calls them off the
//! UI thread. What did not happen says why, in the words the board shows after *{{task}} was not changed.*

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, ModuleId, TaskId},
  name::Name,
  store::{RunStore, TaskStore},
  task::{
    TaskDetails, TaskLimits, TaskUpdate,
    organization::{ChangeActor, TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  messages::{self, Message},
  sharing::Servers,
  tasks::{
    data::{SaveTarget, published_pipelines, save_draft},
    detail::run_store_error,
    list::Readiness,
    run::{QueueFailure, queue_in},
    writer::TaskWriter,
  },
};

/// Sets `task` of the project at `repo_root` to `status` as `actor`'s change, and nothing else.
pub fn set_status_in(
  repo_root: &Path,
  servers: &Servers,
  task: TaskId,
  (status, actor): (WorkflowStatus, ChangeActor),
) -> Result<(), Message> {
  organize_in(repo_root, servers, (task, actor), |value| value.with_status(status))
}

/// Applies `change` to the organization of `task` of the project at `repo_root` as `actor`'s change: its priority or
/// its tags from a card's menu, or its status.
pub fn organize_in(
  repo_root: &Path,
  servers: &Servers,
  (task, actor): (TaskId, ChangeActor),
  change: impl FnOnce(TaskOrganization) -> TaskOrganization,
) -> Result<(), Message> {
  let mut writer = TaskWriter::open(repo_root, servers).map_err(|error| error.message())?;
  writer
    .update_organization_by(task, actor, change)
    .map(|_| ())
    .map_err(|error| error.message())
}

/// Puts `task` of the project at `repo_root` in `module`, or in none, as `actor`'s change: a drop on a column of the
/// board grouped by module.
pub fn set_module_in(
  repo_root: &Path,
  servers: &Servers,
  (task, actor): (TaskId, ChangeActor),
  module: Option<ModuleId>,
) -> Result<(), Message> {
  let mut writer = TaskWriter::open(repo_root, servers).map_err(|error| error.message())?;
  writer
    .set_task_module(task, module, actor)
    .map(|_| ())
    .map_err(|error| error.message())
}

/// Queues the draft `task` on its pipeline's latest published version, as the task page's Run does before it starts the
/// first run, and sets it to Todo; the runner starts it in its turn. A shared task's run claim is taken first and stays
/// with the queued task.
pub fn queue_draft_in(repo_root: &Path, servers: &Servers, task: TaskId, actor: ChangeActor) -> Result<(), Message> {
  let (details, revision, version) = {
    let db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
    let draft = stored(&db, task)?;
    let readiness = Readiness::of(&draft);
    if readiness != Readiness::Ready {
      return Err(readiness.message());
    }
    let published = published_pipelines(&db).map_err(|error| messages::store_error(&error))?;
    let version = published
      .into_iter()
      .find(|pipeline| Some(pipeline.id) == draft.details().pipeline)
      .map(|pipeline| pipeline.version)
      .ok_or_else(|| Message::new("runs.preflight.pipelineNotPublished"))?;
    (draft.details().clone(), draft.revision(), version)
  };
  let queued =
    queue_in(repo_root, servers, SaveTarget::Task(task, revision), details, version).map_err(|failure| first(&failure))?;
  // The queued task keeps the claim for the run the runner starts in its turn.
  drop(queued.claim);
  set_status_in(repo_root, servers, task, (WorkflowStatus::Todo, actor))
}

/// Takes the queued `task`, whose run never started, back to the drafts and sets it to Backlog. A task a reviewed group
/// holds for good leaves its group as *Return to drafts* takes it; one its group still runs in its turn stays.
pub fn return_to_draft_in(repo_root: &Path, servers: &Servers, task: TaskId, actor: ChangeActor) -> Result<(), Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
  if db
    .task_group_for_task(task)
    .map_err(|error| messages::store_error(&error))?
    .is_some()
  {
    db.release_held_tasks(&[task]).map_err(|error| match error {
      StoreError::TaskNotHeld(_) => Message::new("tasks.bulk.notHeld"),
      other => messages::store_error(&other),
    })?;
    drop(db);
    return set_status_in(repo_root, servers, task, (WorkflowStatus::Backlog, actor));
  }
  let has_run = !db
    .runs_of_task(task)
    .map_err(|error| messages::store_error(&run_store_error(error)))?
    .is_empty();
  drop(db);
  let mut writer = TaskWriter::open(repo_root, servers).map_err(|error| error.message())?;
  let mut draft = writer
    .task(task)
    .map_err(|error| error.message())?
    .ok_or_else(|| messages::store_error(&StoreError::TaskNotFound(task)))?;
  draft
    .return_to_draft(has_run)
    .map_err(|_| Message::new("tasks.board.hasRun"))?;
  match writer.save_task(&draft).map_err(|error| error.message())? {
    TaskUpdate::Saved(_) => {}
    TaskUpdate::Conflict { .. } => return Err(messages::task_changed_meanwhile()),
  }
  writer
    .update_organization_by(task, actor, |value| value.with_status(WorkflowStatus::Backlog))
    .map(|_| ())
    .map_err(|error| error.message())
}

/// Adds a draft named `title` to the project at `repo_root`, in Backlog, under `batch`: the board's quick add makes the
/// key once for each draft, so trying again never adds a second one. Everything but its title is filled in on its
/// page. A shared project's draft is made on its server.
pub fn quick_add_in(
  repo_root: &Path,
  servers: &Servers,
  batch: IdempotencyKey,
  title: Name,
) -> Result<orchester_core::task::Task, Message> {
  let details = TaskDetails {
    title,
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let mut writer = TaskWriter::open(repo_root, servers).map_err(|error| error.message())?;
  match save_draft(&mut writer, batch, details, Timestamp::now()).map_err(|error| error.message())? {
    TaskUpdate::Saved(task) => Ok(task),
    TaskUpdate::Conflict { .. } => Err(messages::task_changed_meanwhile()),
  }
}

fn stored(db: &ProjectDb, task: TaskId) -> Result<orchester_core::task::Task, Message> {
  db.task(task)
    .and_then(|found| found.ok_or(StoreError::TaskNotFound(task)))
    .map_err(|error| messages::store_error(&error))
}

/// What a person reads first of why the task was not queued.
fn first(failure: &QueueFailure) -> Message {
  failure
    .messages()
    .into_iter()
    .next()
    .unwrap_or_else(messages::interrupted)
}
