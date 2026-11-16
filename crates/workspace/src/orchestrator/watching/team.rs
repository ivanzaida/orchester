//! A linked project's team tasks, as what a conversation watches lists them. The team server owns a team task's
//! definition and this computer keeps no row of it (`crate::tasks::recorded`), so the watch reads take it from its
//! runs here, and a task that never ran here from the team server's last reading of the watched modules
//! ([`TeamReading`]): its key, its title and the module it is in. Local and team projects list what they lead the
//! same way (`docs/design/local-and-team-parity.md`). A latest run that passed every step and that this computer did
//! not merge is asked of git as the task list asks it: a team mate's computer may have merged it into the module's
//! branch, and then it reads as merged.

use std::{collections::HashSet, path::Path};

use orchester_core::{
  id::TaskId,
  integration::RunWork,
  name::Name,
  run::{Run, RunNumber, TaskPhase},
  store::MergeStore,
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use crate::{
  orchestrator::team_status::{TeamReading, TeamTask},
  tasks::{delivery, recorded},
};

/// A team task with where it stands.
#[derive(Clone, Debug)]
pub(super) struct TeamTaskRead {
  pub task: Task,
  /// Where its latest run stands; `Draft` for a task that never ran here, `None` when the run cannot be read.
  pub phase: Option<TaskPhase>,
  /// The number of its latest run that can be read.
  pub run: Option<RunNumber>,
}

/// The team tasks of `db`'s project, whose checkout is at `repo_root`, that `stored`, its stored tasks, do not hold:
/// those its runs recorded, then those `team` saw that never ran on this computer. Git, run by `programs`, settles
/// the latest runs that passed every step ([`merged_elsewhere`]).
pub(super) fn read(
  (db, repo_root, programs): (&ProjectDb, &Path, &Programs),
  stored: &[Task],
  team: &TeamReading,
) -> Result<Vec<TeamTaskRead>, StoreError> {
  let mut known: HashSet<TaskId> = stored.iter().map(Task::id).collect();
  let mut tasks = Vec::new();
  let mut passed = Vec::new();
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
    if let (Some(TaskPhase::Succeeded), Some(run)) = (phase, latest) {
      passed.push((tasks.len(), RunWork::of(run)));
    }
    tasks.push(TeamTaskRead {
      run: latest.map(Run::number),
      task,
      phase,
    });
  }
  merged_elsewhere((repo_root, programs), &mut tasks, passed);
  for (id, seen) in team.seen() {
    if !known.insert(id) {
      continue;
    }
    let Some(task) = seen_task(id, seen) else {
      continue;
    };
    tasks.push(TeamTaskRead {
      task,
      phase: Some(TaskPhase::Draft),
      run: None,
    });
  }
  Ok(tasks)
}

/// Team task `id` as the team server's last reading `seen` it, for a task that never ran on this computer: its key,
/// its title and its module. `None` for a task with neither a title nor a key the app could have saved.
pub(super) fn seen_task(id: TaskId, seen: &TeamTask) -> Option<Task> {
  // A title the app could not have saved is named by the task's key.
  let title = Name::new(&seen.title)
    .or_else(|_| Name::new(&seen.key.to_string()))
    .ok()?;
  let details = TaskDetails {
    title,
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  // When the server made it is not read: nothing here shows it.
  Some(Task::draft(id, seen.key.clone(), details, Timestamp::from_unix_millis(0)).with_module(seen.module))
}

/// Settles the runs of `passed`, the indexes in `tasks` of latest runs that passed every step with their work, as git
/// in the checkout at `repo_root` tells them: one whose work its branch holds reads as merged.
fn merged_elsewhere(
  (repo_root, programs): (&Path, &Programs),
  tasks: &mut [TeamTaskRead],
  passed: Vec<(usize, RunWork)>,
) {
  if passed.is_empty() {
    return;
  }
  let (indexes, works): (Vec<usize>, Vec<RunWork>) = passed.into_iter().unzip();
  for (index, delivered) in indexes
    .into_iter()
    .zip(delivery::deliveries(programs, repo_root, &works))
  {
    if let Some(read) = tasks.get_mut(index) {
      read.phase = read.phase.map(|phase| delivered.phase(phase));
    }
  }
}
