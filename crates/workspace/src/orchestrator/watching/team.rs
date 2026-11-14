//! A linked project's team tasks, as what a conversation watches lists them. The team server owns a team task's
//! definition and this computer keeps no row of it (`crate::tasks::recorded`), so the watch reads take it from its
//! runs here, and a task that never ran here from the team server's last reading of the watched modules
//! ([`TeamReading`]): its key, its title and the module it is in. Local and team projects list what they lead the
//! same way (`docs/design/local-and-team-parity.md`).

use std::collections::HashSet;

use orchester_core::{
  id::TaskId,
  name::Name,
  run::{Run, RunNumber, TaskPhase},
  store::MergeStore,
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{orchestrator::team_status::TeamReading, tasks::recorded};

/// A team task with where it stands.
#[derive(Clone, Debug)]
pub(super) struct TeamTaskRead {
  pub task: Task,
  /// Where its latest run stands; `Draft` for a task that never ran here, `None` when the run cannot be read.
  pub phase: Option<TaskPhase>,
  /// The number of its latest run that can be read.
  pub run: Option<RunNumber>,
}

/// The team tasks of `db`'s project that `stored`, its stored tasks, do not hold: those its runs recorded, then those
/// `team` saw that never ran on this computer.
pub(super) fn read(db: &ProjectDb, stored: &[Task], team: &TeamReading) -> Result<Vec<TeamTaskRead>, StoreError> {
  let mut known: HashSet<TaskId> = stored.iter().map(Task::id).collect();
  let mut tasks = Vec::new();
  for (task, runs) in recorded::all_with_runs(db)? {
    if !known.insert(task.id()) {
      continue;
    }
    let latest = runs.iter().rev().find_map(|listed| listed.as_ref().ok());
    let merge = match latest {
      Some(run) => db.run_merge(run.id())?,
      None => None,
    };
    let phase = match (latest, runs.last()) {
      (None, Some(_)) => None,
      _ => Some(TaskPhase::of(&task, latest, merge.as_ref())),
    };
    tasks.push(TeamTaskRead {
      run: latest.map(Run::number),
      task,
      phase,
    });
  }
  for (id, seen) in team.seen() {
    if !known.insert(id) {
      continue;
    }
    // A title the app could not have saved is named by the task's key.
    let Ok(title) = Name::new(&seen.title).or_else(|_| Name::new(&seen.key.to_string())) else {
      continue;
    };
    let details = TaskDetails {
      title,
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: None,
      limits: TaskLimits::NONE,
    };
    // When the server made it is not read: nothing here shows it.
    let task = Task::draft(id, seen.key.clone(), details, Timestamp::from_unix_millis(0)).with_module(seen.module);
    tasks.push(TeamTaskRead {
      task,
      phase: Some(TaskPhase::Draft),
      run: None,
    });
  }
  Ok(tasks)
}
