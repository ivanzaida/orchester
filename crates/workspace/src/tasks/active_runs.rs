//! The runs under way of a project, read alone for the sidebar's Running list: the tasks whose latest run is recorded
//! as running, whether each works or waits for the person, the step it is at and how long its agent has worked on
//! it. It reads the runs that are under way and nothing of the others, where [`super::data::ProjectTasks`] reads every
//! task and every run of the project.

use std::{collections::HashSet, path::Path, sync::Arc};

use orchester_core::{
  id::{RunId, TaskId},
  project::TaskKey,
  run::{ActiveClock, Run, TaskPhase},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use super::{recorded, standing::CurrentStep};

/// A task whose latest run is under way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveRun {
  pub task: TaskId,
  pub key: TaskKey,
  pub run: RunId,
  /// Whether the run works ([`TaskPhase::Running`]) or waits for the person ([`TaskPhase::Waiting`]); no other.
  pub phase: TaskPhase,
  /// The step its agent works on, once the run's steps were read.
  pub step: Option<Arc<str>>,
  /// The agent's active time on that step as of the reading, counting on while the run goes.
  pub clock: Option<ActiveClock>,
}

/// The runs under way of the project at `repo_root`, in its tasks' order. Blocks on SQLite.
pub fn load(repo_root: &Path) -> Result<Vec<ActiveRun>, StoreError> {
  read(&ProjectDb::open(repo_root)?)
}

/// [`load`] from `db`, the project database, opened already.
pub fn read(db: &ProjectDb) -> Result<Vec<ActiveRun>, StoreError> {
  let running = db.latest_running_runs()?;
  if running.is_empty() {
    return Ok(Vec::new());
  }
  // A run whose agent waits for a tool decision or an answer waits for a person, as the task list reads it.
  let asking: HashSet<RunId> = db
    .pending_tool_approvals_all()?
    .into_iter()
    .map(|approval| approval.run)
    .chain(db.waiting_questions()?.into_iter().map(|asked| asked.request.run()))
    .collect();
  let now = Timestamp::now();
  let mut runs = Vec::with_capacity(running.len());
  for task in recorded::all(db)? {
    let Some(&(_, id)) = running.iter().find(|(running, _)| *running == task.id()) else {
      continue;
    };
    let run = match db.run(id) {
      Ok(Some(run)) => run,
      Ok(None) => continue,
      Err(RunStoreError::Store(error)) => return Err(error),
      // A run whose record cannot be read is not under way for the list: the task list says it could not be read.
      Err(_) => continue,
    };
    let mut phase = TaskPhase::of(&task, Some(&run), None);
    if phase == TaskPhase::Running && asking.contains(&id) {
      phase = TaskPhase::Waiting;
    }
    if !phase.is_active() {
      continue;
    }
    let (step, clock) = at(db, &run, now)?;
    runs.push(ActiveRun {
      task: task.id(),
      key: task.key().clone(),
      run: id,
      phase,
      step,
      clock,
    });
  }
  Ok(runs)
}

/// The step `run` stands at, by the rule of the task page, and its agent's active time on it as of `now`.
fn at(db: &ProjectDb, run: &Run, now: Timestamp) -> Result<(Option<Arc<str>>, Option<ActiveClock>), StoreError> {
  let Some(current) = CurrentStep::of(run) else {
    return Ok((None, None));
  };
  let execution = run
    .snapshot()
    .body()
    .steps()
    .get(current.number - 1)
    .and_then(|step| run.latest_execution(&step.key))
    .map(|execution| execution.id());
  let clock = execution
    .map(|execution| {
      let invocations = db.invocations(run.id())?;
      let of_step = invocations
        .iter()
        .filter(|invocation| invocation.execution == execution);
      Ok::<_, StoreError>(ActiveClock::of(of_step, now).within(run))
    })
    .transpose()?;
  Ok((Some(current.name), clock))
}
