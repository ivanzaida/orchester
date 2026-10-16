//! A group task whose run fails before it started (its worktree could not be created) holds the group for the
//! person, as a failed task does: retrying its start would only fail the same way.
use std::{fs, num::NonZeroU32};

use orchester_core::{
  run::{RunFailure, RunStatus, WorktreeFailure},
  store::RunStore,
  task_group::{GroupHold, Scheduling},
};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;

use super::{
  reviewed_group,
  support::{Setup, project, scheduler},
};

#[tokio::test(flavor = "multi_thread")]
async fn task_group_run_that_fails_before_it_starts_holds_the_group_for_the_person() {
  let fixture = project(&Setup::new("Append one line."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 3_000_000);
  let (first, second) = (group.tasks()[0], group.tasks()[1]);
  // Git would show the worktree in the person's checkout, so the runner creates none.
  fs::write(root.join(".git/info/exclude"), "").expect("stop excluding .orchester");
  let (scheduler, _events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(4).expect("slots"))
    .await
    .expect("attach");

  for pass in ["first", "next"] {
    scheduler.advance_task_groups().await.expect("advance");
    let run = db
      .run_with_key(first.first_run_key())
      .expect("read")
      .expect("the first task's run is recorded");
    assert_eq!(
      run.status(),
      &RunStatus::Failed(RunFailure::WorktreeNotCreated(WorktreeFailure::NotIgnored)),
      "{pass} pass"
    );
    assert_eq!(
      db.task_group_hold(group.key()).expect("hold"),
      Some(GroupHold::FailedTask),
      "the {pass} pass holds the group for the person, without retrying"
    );
    assert_eq!(db.runs_of_task(first).expect("runs").len(), 1, "no second start");
    assert!(
      db.run_with_key(second.first_run_key()).expect("read").is_none(),
      "the next task waits"
    );
  }
}
