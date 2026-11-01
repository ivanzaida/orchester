//! A first run left pending, as a start that stopped after recording its run leaves it (the runner ended between
//! recording the run and starting it). Nothing in a running runner leaves one otherwise.

use std::{fs, path::Path};

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  run::{BranchName, NewRun, Run},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{queue_another_task, scheduler};

/// Records the first run of the queued `task` without starting it. Its pinned configuration is lent by a run of
/// another queued task that failed before it started, as git did not ignore `.orchester/` for a moment.
pub async fn record_pending_run(root: &Path, task: TaskId) -> Run {
  let lender = queue_another_task(root);
  let exclude = root.join(".git/info/exclude");
  let excluded = fs::read_to_string(&exclude).expect("read exclude");
  fs::write(&exclude, "").expect("stop excluding .orchester");
  let (scheduler, _events) = scheduler(root);
  let refused = scheduler.start_run(IdempotencyKey::generate(), lender).await;
  let Err(Rejection::RunNotStarted { run: lent, .. }) = refused else {
    panic!("the lending run fails before it starts, got {refused:?}");
  };
  fs::write(&exclude, excluded).expect("exclude .orchester again");
  let mut db = ProjectDb::open(root).expect("open");
  let lent = db.run(lent).expect("read").expect("lending run");
  let task = db.task(task).expect("task").expect("exists");
  let limits = task.details().limits;
  db.create_run(
    task.id().first_run_key(),
    NewRun {
      task: task.id(),
      snapshot: lent.snapshot().clone(),
      branch: BranchName::for_next_run(task.key(), 0),
      base: lent.base().clone(),
      base_branch: lent.base_branch().cloned(),
      base_source: lent.base_source(),
      budget: limits.budget(),
      max_duration: limits.max_duration(),
    },
    Timestamp::now(),
  )
  .expect("record the run without starting it")
}
