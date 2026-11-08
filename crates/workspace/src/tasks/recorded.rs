//! A team task as its runs recorded it. The team server owns a team task's definition and this computer keeps none it
//! may trust: the copy the old sharing stream wrote is missing before the first sync and when signed out, and may be
//! stale. What a team task's records need — its key, its title, its brief — they get from the inputs its newest run
//! started with (`RunInputs`), which never change. A local task's definition is its stored row.

use orchester_core::{
  id::TaskId,
  project::TaskKey,
  store::{RunStore, TaskStore},
  task::{
    Task, TaskDetails,
    identity::{TaskIdentity, TaskOwner},
  },
};
use orchester_store::{ProjectDb, StoreError};

use super::detail::run_store_error;

/// The task `id` as the project's records need it: a local task as stored; a team task as its newest run that kept
/// inputs recorded it when it started (its title and brief then, not now), else — a run of the old sharing stream
/// kept none — the row the stream cached. `None` when no record defines it.
pub fn task_of(db: &ProjectDb, id: TaskId) -> Result<Option<Task>, StoreError> {
  let team = matches!(
    db.task_identity(id)?,
    Some(TaskIdentity {
      owner: TaskOwner::Team { .. },
      ..
    })
  );
  if team && let Some(task) = recorded(db, id)? {
    return Ok(Some(task));
  }
  db.task(id)
}

/// The task as the newest of its runs that kept inputs recorded it.
fn recorded(db: &ProjectDb, id: TaskId) -> Result<Option<Task>, StoreError> {
  let runs = db.runs_of_task(id).map_err(run_store_error)?;
  for run in runs.iter().rev().filter_map(|listed| listed.as_ref().ok()) {
    let Some(inputs) = db.run_inputs(run.id())? else {
      continue;
    };
    let bundle = inputs.bundle();
    let at_start = bundle.task();
    let details = TaskDetails {
      title: bundle.task_title(),
      brief: at_start.brief.clone(),
      acceptance_criteria: at_start.criteria.clone(),
      pipeline: Some(run.snapshot().pipeline().resource),
      limits: bundle.limits(),
    };
    let key = TaskKey::new(db.project().key().clone(), bundle.task_number());
    return Ok(Some(Task::draft(id, key, details, run.created_at())));
  }
  Ok(None)
}
