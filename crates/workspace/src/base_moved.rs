//! When the base branch moves, the working agents are told to merge it (ORC-54, `orchester_core::base_move`). Each
//! time the Orchestrator chat follows the project (once per follow, outside its lock, as it asks git about merges)
//! and right after Orchester merged a run, the base branches of the project's unfinished runs are read in the user's
//! checkout. A branch seen at a new head is a move, whether Orchester's merge, a commit, a pull or a fast-forward
//! made it; moves within two minutes of the first are one. For each unfinished run on a branch that moved and whose
//! branch lacks the new head, a notice is queued in place of the run's pending one: what landed, and the files both
//! sides changed. The runner delivers it to the run's next agent of a step that implements the change, never a Review
//! or Verify step (`orchester-runner`, `orchester_core::resource::Pipeline::takes_base_moves`). With the project's
//! switch off, the heads are still kept, so that turning it on does not tell of old moves.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  base_move::{BaseHead, BaseNotice, NoticeDelivery, Observed, deliver_after, landed_tasks},
  id::NoticeId,
  integration::{GitError, branch_head, drift},
  revision::Revision,
  run::{BranchName, Run, RunStatus},
  store::{MergeStore, RunStore, TaskStore, WorktreeStore},
  task::Task,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use crate::{
  messages::{self, Message},
  tasks::{
    changes::GIT,
    delivery::{self, Delivery},
    detail::run_store_error,
  },
};

/// Reads the base branches of the unfinished runs of the project at `root` with `programs`' git, records where each
/// is, and queues a notice for each run a move left behind, unless the project turned that off. Gives the number of
/// notices queued. A branch git cannot read is logged and read again next time. Blocks on SQLite and git.
pub fn observe_in(root: &Path, programs: &Programs) -> Result<usize, Message> {
  observe(root, programs, Timestamp::now()).map_err(|error| messages::store_error(&error))
}

/// [`observe_in`] right after Orchester merged a run into its base branch in the checkout at `root`. The merge stands
/// whatever this finds: a failure is logged, and the next follow reads the branch again.
pub fn after_merge(root: &Path, programs: &Programs) {
  match observe(root, programs, Timestamp::now()) {
    Ok(queued) => tracing::info!(queued, "the runs on the merged base branch were told it moved"),
    Err(error) => tracing::warn!(%error, "the runs on the merged base branch could not be told it moved yet"),
  }
}

/// A base branch, and the unfinished runs on it with their tasks.
type OnBase<'a> = (BranchName, Vec<(Run, &'a Task)>);

fn observe(root: &Path, programs: &Programs, now: Timestamp) -> Result<usize, StoreError> {
  let mut db = ProjectDb::open(root)?;
  let tasks = db.tasks()?;
  let mut by_base: BTreeMap<String, OnBase> = BTreeMap::new();
  for task in &tasks {
    let Some(run) = unfinished_run(&db, task)? else {
      continue;
    };
    let Some(base) = run.base_branch().cloned() else {
      continue;
    };
    by_base
      .entry(base.as_str().to_owned())
      .or_insert_with(|| (base, Vec::new()))
      .1
      .push((run, task));
  }
  let tells = db.tells_base_moves()?;
  let mut queued = 0;
  for (base, runs) in by_base.into_values() {
    let moved = match moved(&mut db, (root, programs), &base, now) {
      Ok(Some(moved)) => moved,
      Ok(None) => continue,
      Err(Unread::Store(error)) => return Err(error),
      Err(Unread::Git(error)) => {
        tracing::warn!(%error, branch = %base, "cannot read where the base branch is; it is read again later");
        continue;
      }
    };
    if !tells {
      continue;
    }
    for (run, task) in runs {
      if left_behind_notice(&mut db, (root, programs), (&run, task, &tasks), (&base, &moved), now)? {
        queued += 1;
      }
    }
  }
  Ok(queued)
}

/// What kept a base branch from being read.
enum Unread {
  Store(StoreError),
  Git(GitError),
}

/// Where `base` is now and when the move that put it there began, when that is a move since it was seen last;
/// recorded either way.
fn moved(
  db: &mut ProjectDb,
  (root, programs): (&Path, &Programs),
  base: &BranchName,
  now: Timestamp,
) -> Result<Option<Moved>, Unread> {
  let Some(head) = branch_head(&programs.runner(), Path::new(GIT), (root, base)).map_err(Unread::Git)? else {
    return Ok(None);
  };
  let seen = db.base_head(base).map_err(Unread::Store)?;
  let (seen, moved) = match BaseHead::observe(seen.as_ref(), head, now) {
    Observed::Unchanged => return Ok(None),
    Observed::First(head) => (head, None),
    Observed::Moved { head, moved_at } => {
      let moved = Moved {
        head: head.head.clone(),
        moved_at,
      };
      (head, Some(moved))
    }
  };
  db.save_base_head(base, &seen).map_err(Unread::Store)?;
  Ok(moved)
}

/// A move of a base branch: where it is now, and when the move began.
struct Moved {
  head: Revision,
  moved_at: Timestamp,
}

/// `task`'s latest run while it is unfinished: it may still go on (it works, waits, paused, was interrupted, or
/// succeeded and a follow-up can ask its agent again), its work was not merged and its worktree is there.
fn unfinished_run(db: &ProjectDb, task: &Task) -> Result<Option<Run>, StoreError> {
  let Some(Ok(run)) = db.runs_of_task(task.id()).map_err(run_store_error)?.pop() else {
    return Ok(None);
  };
  if matches!(run.status(), RunStatus::Failed(_) | RunStatus::Cancelled) {
    return Ok(None);
  }
  if db.run_merge(run.id())?.is_some() || db.worktree_removal(run.id())?.is_some() {
    return Ok(None);
  }
  Ok(Some(run))
}

/// Queues a notice for `run`, of `task` (among the project's `tasks`), that `base` moved to `moved`, when its branch
/// lacks that head; whether it did. A run that succeeded and whose work git tells merged outside Orchester, or that
/// has nothing to merge, is told nothing. What git cannot tell of the run is logged and the run is left out.
fn left_behind_notice(
  db: &mut ProjectDb,
  (root, programs): (&Path, &Programs),
  (run, task, tasks): (&Run, &Task, &[Task]),
  (base, moved): (&BranchName, &Moved),
  now: Timestamp,
) -> Result<bool, StoreError> {
  if run.status() == &RunStatus::Succeeded && delivery::of_run(programs, root, run) != Delivery::Waiting {
    return Ok(false);
  }
  let found = drift(&programs.runner(), Path::new(GIT), (root, run.branch(), &moved.head));
  let drift = match found {
    Ok(Some(drift)) => drift,
    Ok(None) => return Ok(false),
    Err(error) => {
      tracing::warn!(%error, run = %run.id(), "cannot tell what the run's branch lacks of its base; it is not told");
      return Ok(false);
    }
  };
  let named = tasks
    .iter()
    .filter(|other| other.id() != task.id())
    .map(|other| (other.key().to_string(), other.details().title.as_str().to_owned()));
  let notice = BaseNotice {
    id: NoticeId::generate(),
    run: run.id(),
    task: task.id(),
    branch: base.clone(),
    revision: moved.head.clone(),
    tasks: landed_tasks(&drift.commits, named),
    commits: drift.commits,
    overlapping: drift.overlapping,
    noticed_at: now,
    deliver_after: deliver_after(moved.moved_at),
    delivery: NoticeDelivery::Pending,
  };
  db.queue_base_notice(&notice)?;
  tracing::info!(run = %run.id(), branch = %base, revision = %moved.head, "the run is told that its base moved");
  Ok(true)
}
