//! Deleting a task (ORC-91), a person's decision only: the task goes with its runs and their history, its
//! conversation, artifacts and memory records ([`orchester_store::ProjectDb::delete_task`]), and with what its runs
//! left in git. Their worktrees are removed and their merged branches deleted, as Remove worktree does; worktrees
//! with uncommitted changes and branches with commits merged nowhere stay unless the person also chose to remove
//! unmerged work. Merged history is never touched: a merged branch's commits stay on the branch they were merged
//! into. A run that has not ended keeps its task (stop it first), as does a local integration that still holds a
//! run's worktree; a shared project's tasks are the team server's, which keeps no deletion.
//!
//! Git goes first and the database after: should git refuse a worktree, the task stays, with the worktrees removed
//! so far recorded as removed, and deleting again goes on from there. A run whose record cannot be read is deleted
//! with the task; what it left in git is not found. Blocks on SQLite and git; the UI calls it off the UI thread.

use std::path::Path;

use orchester_core::{
  id::TaskId,
  integration::{Cleanup, CleanupError, KeepReason, Leftovers, RemovalStart, clean_up, clean_up_all, leftovers_of},
  run::Run,
  store::{RunStore, WorktreeStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError, TaskDeletionFacts};

use super::{
  changes::GIT,
  detail::run_store_error,
  integration::{IntegrationProblem, prune_indexes},
};
use crate::messages::{self, Message};

/// What deleting a task takes with it, read for its confirmation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeletionPreview {
  pub facts: TaskDeletionFacts,
  /// What its runs left that goes either way: worktrees without uncommitted changes, and merged branches.
  pub worktrees: usize,
  pub merged_branches: usize,
  /// What stays unless the person also removes unmerged work: worktrees with uncommitted changes, and branches with
  /// commits merged nowhere.
  pub changed_worktrees: usize,
  pub unmerged_branches: usize,
}

impl DeletionPreview {
  /// Whether the runs left work that is not merged, which the person may choose to remove too.
  pub const fn has_unmerged_work(&self) -> bool {
    self.changed_worktrees > 0 || self.unmerged_branches > 0
  }
}

/// What deleting `task` of the project at `repo_root` takes with it; why it cannot be deleted now, if so.
pub fn preview_in(repo_root: &Path, programs: &Programs, task: TaskId) -> Result<DeletionPreview, Message> {
  let db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
  check_deletable(&db, task)?;
  let facts = db
    .task_deletion_facts(task)
    .map_err(|error| messages::store_error(&error))?;
  let runs = readable_runs(&db, task)?;
  let listed: Vec<&Run> = runs.iter().collect();
  let left = leftovers_of(&programs.runner(), Path::new(GIT), repo_root, &listed)
    .map_err(|error| IntegrationProblem::Read(error).message(""))?;
  Ok(left.iter().fold(
    DeletionPreview {
      facts,
      ..DeletionPreview::default()
    },
    counted,
  ))
}

/// `preview` with what one run left.
fn counted(mut preview: DeletionPreview, left: &Leftovers) -> DeletionPreview {
  for worktree in left.worktrees.iter().filter(|worktree| !worktree.missing) {
    if worktree.loses_files() {
      preview.changed_worktrees += 1;
    } else {
      preview.worktrees += 1;
    }
  }
  for branch in &left.branches {
    if branch.merged {
      preview.merged_branches += 1;
    } else {
      preview.unmerged_branches += 1;
    }
  }
  preview
}

/// Deletes `task` of the project at `repo_root`: what its runs left in git first, with unmerged work only when
/// `unmerged`, then the task with everything recorded about it.
pub fn delete_in(repo_root: &Path, programs: &Programs, task: TaskId, unmerged: bool) -> Result<(), Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
  check_deletable(&db, task)?;
  let runs = readable_runs(&db, task)?;
  let at = Timestamp::now();
  hold_worktrees(&mut db, &runs, at)?;
  let git = programs.git_work();
  for run in &runs {
    let cleanup = if unmerged {
      clean_up_all(&git, Path::new(GIT), repo_root, run)
    } else {
      clean_up(&git, Path::new(GIT), repo_root, run, None)
    };
    refused_by_git(cleanup)?;
  }
  prune_indexes(repo_root);
  db.delete_task(task, at).map_err(|error| delete_error(&error))
}

/// Refuses a task of a shared project, and one whose run has not ended.
fn check_deletable(db: &ProjectDb, task: TaskId) -> Result<(), Message> {
  if db.sharing().map_err(|error| messages::store_error(&error))?.is_some() {
    return Err(delete_error(&StoreError::SharedProject));
  }
  match db.active_task_run(task) {
    Ok(None) => Ok(()),
    Ok(Some(run)) => Err(delete_error(&StoreError::TaskRunActive(run))),
    Err(error) => Err(messages::store_error(&error)),
  }
}

/// The runs of `task` whose record can be read.
fn readable_runs(db: &ProjectDb, task: TaskId) -> Result<Vec<Run>, Message> {
  let listed = db
    .runs_of_task(task)
    .map_err(|error| messages::store_error(&run_store_error(error)))?;
  Ok(listed.into_iter().filter_map(Result::ok).collect())
}

/// Records the removal of every run's worktrees, so none of them continues while they go; refused, with nothing
/// recorded, while a local integration still holds one of them.
fn hold_worktrees(db: &mut ProjectDb, runs: &[Run], at: Timestamp) -> Result<(), Message> {
  let mut recorded = Vec::new();
  for run in runs {
    let start = db
      .begin_worktree_removal(run.id(), at)
      .map_err(|error| messages::store_error(&error));
    let refusal = match start {
      Ok(RemovalStart::Recorded) => {
        recorded.push(run.id());
        continue;
      }
      Ok(RemovalStart::AlreadyRecorded) => continue,
      Ok(RemovalStart::LiveWork) => IntegrationProblem::Cleanup(CleanupError::LiveWork).message(""),
      Ok(RemovalStart::LocalIntegration) => IntegrationProblem::Cleanup(CleanupError::LocalIntegration).message(""),
      Err(message) => message,
    };
    for run in recorded {
      if let Err(error) = db.undo_worktree_removal(run, at) {
        tracing::warn!(%error, %run, "a worktree removal recorded for a task's deletion stays recorded");
      }
    }
    return Err(refusal);
  }
  Ok(())
}

/// Why git kept something the deletion was to remove: it refused or did not finish. A worktree with uncommitted
/// changes or a branch not merged, which the person chose to keep, is no refusal.
fn refused_by_git(cleanup: Result<Cleanup, CleanupError>) -> Result<(), Message> {
  let cleanup = cleanup.map_err(|error| IntegrationProblem::Cleanup(error).message(""))?;
  if let Some((worktree, KeepReason::Failed(detail))) = cleanup
    .kept_worktrees
    .into_iter()
    .find(|(_, reason)| matches!(reason, KeepReason::Failed(_)))
  {
    return Err(
      IntegrationProblem::NotRemoved {
        path: worktree.path,
        detail,
      }
      .message(""),
    );
  }
  match cleanup
    .kept_branches
    .into_iter()
    .find(|(_, reason)| matches!(reason, KeepReason::Failed(_)))
  {
    Some((branch, KeepReason::Failed(detail))) => Err(
      Message::new("tasks.delete.branchNotDeleted")
        .with("branch", branch.to_string())
        .with("detail", detail),
    ),
    _ => Ok(()),
  }
}

/// What a person reads when the task was not deleted.
fn delete_error(error: &StoreError) -> Message {
  match error {
    StoreError::SharedProject | StoreError::SharedTask(_) | StoreError::NotSharedTask(_) => {
      Message::new("tasks.delete.shared")
    }
    StoreError::TaskRunActive(_) => Message::new("tasks.delete.stopRunFirst"),
    other => messages::store_error(other),
  }
}
