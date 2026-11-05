//! What a reading of the project's tasks ([`super::data::ProjectTasks::read`]) knows of their runs, read for the whole
//! project at once: every run of each task, and what the list shows of a latest run beside the run itself, so that a
//! reading takes the same statements however many tasks and runs the project has (ORC-742).

use std::collections::{HashMap, HashSet};

use orchester_core::{
  id::{RunId, TaskId},
  integration::RunMerge,
  run::{ListedRun, Run, RunEvent, RunStatus},
  store::{MergeStore, RunStore, WorktreeStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use super::detail::run_store_error;

/// The project's runs and what the task list reads beside them.
#[derive(Default)]
pub(super) struct ProjectRuns {
  /// Every run of each task that has one, by number.
  runs: HashMap<TaskId, Vec<ListedRun<RunStoreError>>>,
  /// Each task's first run, the one its reviewed group schedules ([`ProjectDb::task_group_for_run`]).
  first: HashMap<TaskId, RunId>,
  /// When each run's last event was recorded.
  last_event_at: HashMap<RunId, Timestamp>,
  merges: HashMap<RunId, RunMerge>,
  worktree_removed: HashSet<RunId>,
  completed: HashSet<RunId>,
  /// Each task's latest run that has not ended, as the store decides it ([`ProjectDb::active_task_run`]).
  active: HashMap<TaskId, RunId>,
  /// When each task's organization last changed, once it was stored.
  organization_updated: HashMap<TaskId, Timestamp>,
  /// The blocking tasks of each task that have not finished; a task left out has none.
  blocking: HashMap<TaskId, Vec<TaskId>>,
  /// The tag names each *Not shared* task kept.
  kept_tags: HashMap<TaskId, Vec<String>>,
}

/// What the task list shows of a latest run beside the run itself.
pub(super) struct RunEnds {
  /// When it last changed: its last recorded event, else when it was created.
  pub(super) updated_at: Timestamp,
  pub(super) merge: Option<RunMerge>,
  /// Interrupted, and a person removed its worktree (ORC-63).
  pub(super) worktree_removed: bool,
  /// Succeeded, and completed without a merge (ORC-93).
  pub(super) completed: bool,
}

impl RunEnds {
  /// Of `run`, read on its own.
  pub(super) fn read(db: &ProjectDb, run: &Run) -> Result<Self, StoreError> {
    Ok(Self {
      updated_at: db
        .events(run.id())
        .map_err(run_store_error)?
        .last()
        .map_or_else(|| run.created_at(), RunEvent::at),
      merge: db.run_merge(run.id())?,
      worktree_removed: matches!(run.status(), RunStatus::Interrupted(_)) && db.worktree_removal(run.id())?.is_some(),
      completed: run.status() == &RunStatus::Succeeded && db.run_completion(run.id())?.is_some(),
    })
  }
}

impl ProjectRuns {
  /// Of every task of `db`, in a constant number of statements.
  pub(super) fn read(db: &ProjectDb) -> Result<Self, StoreError> {
    Ok(Self {
      runs: db.runs_by_task().map_err(run_store_error)?,
      first: db.first_runs()?,
      last_event_at: db.last_event_times()?,
      merges: db.run_merges()?,
      worktree_removed: db.runs_with_worktree_removed()?,
      completed: db.completed_runs()?,
      active: db.active_task_runs()?,
      organization_updated: db.task_organization_update_times()?,
      blocking: db.unfinished_blocking_by_task()?,
      kept_tags: db.kept_tag_names()?,
    })
  }

  /// Of `tasks` and the *Not shared* tasks `not_shared`, read task by task as the reading did before ORC-742: what
  /// the tests compare [`Self::read`] with.
  #[cfg(feature = "test-support")]
  pub(super) fn read_per_task(db: &ProjectDb, tasks: &[Task], not_shared: &[Task]) -> Result<Self, StoreError> {
    let mut read = Self::default();
    for task in tasks.iter().chain(not_shared) {
      let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
      if let Some(Ok(latest)) = runs.last() {
        read.note_latest(db, task, latest)?;
      }
      if let Some(active) = db.active_task_run(task.id())? {
        read.active.insert(task.id(), active);
      }
      if !runs.is_empty() {
        read.runs.insert(task.id(), runs);
      }
    }
    for task in tasks {
      if let Some(updated) = db.task_organization_updated_at(task.id())? {
        read.organization_updated.insert(task.id(), updated);
      }
      read
        .blocking
        .insert(task.id(), db.unfinished_blocking_tasks(task.id())?);
    }
    for task in not_shared {
      if let Some(names) = db.shared_task_info(task.id())?.and_then(|info| info.kept_tags) {
        read.kept_tags.insert(task.id(), names);
      }
    }
    Ok(read)
  }

  /// Keeps what [`Self::ends`] and [`Self::is_first_run`] tell of `latest`, the latest run of `task`, read on its own.
  #[cfg(feature = "test-support")]
  fn note_latest(&mut self, db: &ProjectDb, task: &Task, latest: &Run) -> Result<(), StoreError> {
    if let Some(first) = db.run_with_key(task.id().first_run_key()).map_err(run_store_error)? {
      self.first.insert(task.id(), first.id());
    }
    let ends = RunEnds::read(db, latest)?;
    self.last_event_at.insert(latest.id(), ends.updated_at);
    if let Some(merge) = ends.merge {
      self.merges.insert(latest.id(), merge);
    }
    if ends.worktree_removed {
      self.worktree_removed.insert(latest.id());
    }
    if ends.completed {
      self.completed.insert(latest.id());
    }
    Ok(())
  }

  /// The runs of `task`, by number.
  pub(super) fn runs_of(&self, task: TaskId) -> &[ListedRun<RunStoreError>] {
    self.runs.get(&task).map_or(&[], Vec::as_slice)
  }

  /// Whether `latest`, a readable run of `task`, is the task's first run, the one its reviewed group schedules
  /// ([`ProjectDb::task_group_for_run`]). A first run that cannot be read fails the reading, as it fails that.
  pub(super) fn is_first_run(&self, db: &ProjectDb, task: &Task, latest: &Run) -> Result<bool, StoreError> {
    let Some(first) = self.first.get(&task.id()) else {
      return Ok(false);
    };
    if *first == latest.id() {
      return Ok(true);
    }
    let unreadable = self
      .runs_of(task.id())
      .iter()
      .any(|run| run.as_ref().is_err_and(|unreadable| unreadable.id == *first));
    if unreadable {
      // Reading it again gives its error.
      db.run_with_key(task.id().first_run_key()).map_err(run_store_error)?;
    }
    Ok(false)
  }

  /// What the list shows of `run` beside the run itself.
  pub(super) fn ends(&self, run: &Run) -> RunEnds {
    RunEnds {
      updated_at: self
        .last_event_at
        .get(&run.id())
        .copied()
        .unwrap_or_else(|| run.created_at()),
      merge: self.merges.get(&run.id()).cloned(),
      worktree_removed: self.worktree_removed.contains(&run.id()),
      completed: self.completed.contains(&run.id()),
    }
  }

  /// Whether a run of `task` has not ended, as the store decides it.
  pub(super) fn is_active(&self, task: TaskId) -> bool {
    self.active.contains_key(&task)
  }

  /// When `task`'s organization last changed, else when it was created.
  pub(super) fn organization_updated(&self, task: &Task) -> Timestamp {
    self
      .organization_updated
      .get(&task.id())
      .copied()
      .unwrap_or(task.created_at())
  }

  /// The blocking tasks of `task` that have not finished.
  pub(super) fn blocking(&self, task: TaskId) -> Vec<TaskId> {
    self.blocking.get(&task).cloned().unwrap_or_default()
  }

  /// The tag names *Not shared* `task` kept, if it kept any.
  pub(super) fn kept_tags(&self, task: TaskId) -> Option<&Vec<String>> {
    self.kept_tags.get(&task)
  }
}
