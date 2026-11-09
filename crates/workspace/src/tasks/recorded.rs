//! A team task as its runs recorded it. The team server owns a team task's definition and this computer keeps none it
//! may trust: the copy the old sharing stream wrote is missing before the first sync and when signed out, and may be
//! stale. What a team task's records need — its key, its title, its brief — they get from the inputs its newest run
//! started with (`RunInputs`), which never change. A local task's definition is its stored row.

use orchester_core::{
  id::TaskId,
  project::TaskKey,
  run::{ListedRun, Run},
  store::{RunStore, TaskStore},
  task::{
    Task, TaskDetails,
    identity::{TaskIdentity, TaskOwner},
  },
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use super::detail::run_store_error;

/// The task `id` as the project's records need it: a local task as stored; a team task as its newest run that kept
/// inputs recorded it when it started (its title and brief then, not now). A team task whose runs here kept none — runs
/// of the old sharing stream — is the row the stream cached; one with no run here is not defined by any record of this
/// computer. `None` when no record defines it.
pub fn task_of(db: &ProjectDb, id: TaskId) -> Result<Option<Task>, StoreError> {
  if !is_team(db, id)? {
    return db.task(id);
  }
  let runs = db.runs_of_task(id).map_err(run_store_error)?;
  if let Some(task) = recorded(db, id, &runs)? {
    return Ok(Some(task));
  }
  if runs.is_empty() { Ok(None) } else { db.task(id) }
}

fn is_team(db: &ProjectDb, id: TaskId) -> Result<bool, StoreError> {
  Ok(matches!(
    db.task_identity(id)?,
    Some(TaskIdentity {
      owner: TaskOwner::Team { .. },
      ..
    })
  ))
}

/// The task as the newest of its `runs` that kept inputs recorded it.
fn recorded(db: &ProjectDb, id: TaskId, runs: &[ListedRun<RunStoreError>]) -> Result<Option<Task>, StoreError> {
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

/// The project's tasks as [`task_of`] gives them: the stored tasks (a team task as its runs recorded it) and the
/// team tasks this computer keeps no definition of but their runs. A team task no record of this computer defines is
/// left out; its runs, if any, are [`unrecorded_runs`].
pub fn all(db: &ProjectDb) -> Result<Vec<Task>, StoreError> {
  let mut tasks = Vec::new();
  for stored in db.tasks()? {
    tasks.extend(task_of(db, stored.id())?);
  }
  for id in db.team_task_ids()? {
    if !tasks.iter().any(|task| task.id() == id)
      && let Some(task) = task_of(db, id)?
    {
      tasks.push(task);
    }
  }
  Ok(tasks)
}

/// The readable runs of team tasks that no record of this computer defines: a run of the old sharing stream that kept
/// neither inputs nor a cached row of its task, so its task's key and title are unknown here. Oldest first.
pub fn unrecorded_runs(db: &ProjectDb) -> Result<Vec<Run>, StoreError> {
  let mut unrecorded = Vec::new();
  for id in db.team_task_ids()? {
    if task_of(db, id)?.is_none() {
      let runs = db.runs_of_task(id).map_err(run_store_error)?;
      unrecorded.extend(runs.into_iter().flatten());
    }
  }
  Ok(unrecorded)
}
