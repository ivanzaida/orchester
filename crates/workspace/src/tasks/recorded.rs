//! A team task as its runs recorded it. The team server owns a team task's definition and this computer keeps none it
//! may trust: the copy the old sharing stream wrote is missing before the first sync and when signed out, and may be
//! stale. What a team task's records need — its key, its title, its brief — they get from the inputs its newest run
//! started with (`RunInputs`), which never change. A local task's definition is its stored row.

use std::collections::HashSet;

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
use orchester_store::{ProjectDb, RunStoreError, StoreError, TeamRun};

use super::detail::run_store_error;

/// The task `id` as the project's records need it: a local task as stored; a team task as its newest run that kept
/// inputs recorded it when it started (its title and brief then, not now). A team task whose runs here kept none — runs
/// of the old sharing stream — is the row the stream cached; one with no run here is not defined by any record of this
/// computer. `None` when no record defines it.
pub fn task_of(db: &ProjectDb, id: TaskId) -> Result<Option<Task>, StoreError> {
  if is_team(db, id)? {
    team_task_of(db, id)
  } else {
    db.task(id)
  }
}

/// [`task_of`] for the team task `id`.
fn team_task_of(db: &ProjectDb, id: TaskId) -> Result<Option<Task>, StoreError> {
  Ok(team_task_with_runs(db, id)?.map(|(task, _)| task))
}

/// A task with its runs, oldest first, as they were read for it.
pub type WithRuns = (Task, Vec<ListedRun<RunStoreError>>);

/// [`team_task_of`] with the runs it was read from, which a caller need not decode again.
fn team_task_with_runs(db: &ProjectDb, id: TaskId) -> Result<Option<WithRuns>, StoreError> {
  let runs = db.runs_of_task(id).map_err(run_store_error)?;
  if let Some(task) = recorded(db, id, &runs)? {
    return Ok(Some((task, runs)));
  }
  if runs.is_empty() {
    return Ok(None);
  }
  Ok(db.task(id)?.map(|task| (task, runs)))
}

/// Whether team task `id` is owned by the team server (an identity row says so), whether or not a definition is kept.
pub fn is_team(db: &ProjectDb, id: TaskId) -> Result<bool, StoreError> {
  Ok(matches!(
    db.task_identity(id)?,
    Some(TaskIdentity {
      owner: TaskOwner::Team { .. },
      ..
    })
  ))
}

/// Where a run's work goes: told by the run itself, not by whether its project is linked to a team server and not by
/// its task: a task the old sharing stream cached has the team's identity too, and its runs are local ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunRoute {
  /// A run with neither a team record nor team inputs: merged, continued and messaged on this computer, whatever its
  /// task.
  Local,
  /// A team run with its team record: its work is reported under the claim it started with.
  Team,
  /// A team run with no team record, whose start stopped before the record was written: no claim or work to report
  /// under, so it takes neither route.
  TeamUnrecorded,
}

/// The route of `run`: a team run when it has a team record, or when it was created with inputs the team server
/// accepted and kept no record; a local run otherwise.
pub fn route_of_run(db: &ProjectDb, run: &Run) -> Result<RunRoute, StoreError> {
  route_with_record(db, run, db.team_run(run.id())?.as_ref())
}

/// [`route_of_run`] for a caller that has read the team record `record` of `run` already.
pub fn route_with_record(db: &ProjectDb, run: &Run, record: Option<&TeamRun>) -> Result<RunRoute, StoreError> {
  Ok(if record.is_some() {
    RunRoute::Team
  } else if db.has_team_inputs(run.id())? {
    RunRoute::TeamUnrecorded
  } else {
    RunRoute::Local
  })
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
  Ok(among(db, |_| true)?.into_iter().map(|(task, _)| task).collect())
}

/// [`all`], each task with its runs, oldest first: for a reading that needs every task's runs, so a team task's runs
/// are decoded once, for its record and for the caller, not once more by `runs_of_task`.
pub fn all_with_runs(db: &ProjectDb) -> Result<Vec<WithRuns>, StoreError> {
  among(db, |_| true)?
    .into_iter()
    .map(|(task, runs)| match runs {
      Some(runs) => Ok((task, runs)),
      None => db
        .runs_of_task(task.id())
        .map(|runs| (task, runs))
        .map_err(run_store_error),
    })
    .collect()
}

/// [`all`] of only the tasks in `ids`, in the same order: what a reading that knows which tasks it needs pays for, not
/// the runs of every team task.
pub fn of(db: &ProjectDb, ids: &HashSet<TaskId>) -> Result<Vec<Task>, StoreError> {
  Ok(
    among(db, |id| ids.contains(&id))?
      .into_iter()
      .map(|(task, _)| task)
      .collect(),
  )
}

/// A task [`among`] reads, with the runs its record was read from when it is a team task.
type Among = (Task, Option<Vec<ListedRun<RunStoreError>>>);

/// [`all`] of the tasks `wanted` names. One query names the team tasks, so a local task costs no read beyond its stored
/// row, and a team task not wanted costs none of its runs. A team task comes with the runs its record was read from, a
/// local task with none read.
fn among(db: &ProjectDb, wanted: impl Fn(TaskId) -> bool) -> Result<Vec<Among>, StoreError> {
  let team = db.team_task_ids()?;
  let mut tasks = Vec::new();
  let mut stored_ids = HashSet::new();
  for stored in db.tasks()? {
    stored_ids.insert(stored.id());
    if !wanted(stored.id()) {
      continue;
    }
    if team.contains(&stored.id()) {
      tasks.extend(team_task_with_runs(db, stored.id())?.map(|(task, runs)| (task, Some(runs))));
    } else {
      tasks.push((stored, None));
    }
  }
  for id in team {
    if wanted(id)
      && !stored_ids.contains(&id)
      && let Some((task, runs)) = team_task_with_runs(db, id)?
    {
      tasks.push((task, Some(runs)));
    }
  }
  Ok(tasks)
}

/// The runs of team tasks that no record of this computer defines, readable or not: a run of the old sharing stream
/// that kept neither inputs nor a cached row of its task, so its task's key and title are unknown here. Oldest first
/// within a task.
pub fn unrecorded_listed(db: &ProjectDb) -> Result<Vec<ListedRun<RunStoreError>>, StoreError> {
  let mut unrecorded = Vec::new();
  for id in db.team_task_ids()? {
    if task_of(db, id)?.is_none() {
      unrecorded.extend(db.runs_of_task(id).map_err(run_store_error)?);
    }
  }
  Ok(unrecorded)
}

/// [`unrecorded_listed`] without the runs that cannot be read, for what only a run that can be read can be doing: a run
/// that cannot be read cannot be working or waiting.
pub fn unrecorded_runs(db: &ProjectDb) -> Result<Vec<Run>, StoreError> {
  Ok(unrecorded_listed(db)?.into_iter().flatten().collect())
}
