//! A person's hold on new starts: held in the whole project or in one group, no first run starts, from the
//! group's turn or on request, the runner may go idle, and the group's rows say why; released, the group starts.
use std::num::NonZeroU32;

use orchester_core::{
  store::RunStore,
  task_group::{GroupHold, HoldScope, Scheduling},
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Rejection};
use orchester_store::ProjectDb;

use super::{
  becomes_idle, reviewed_group,
  support::{Setup, project, scheduler},
};

#[tokio::test(flavor = "multi_thread")]
async fn task_group_held_queue_starts_nothing_until_released() {
  let fixture = project(&Setup::new("Append one line."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(
    &mut db,
    fixture.task,
    Scheduling::Parallel(NonZeroU32::new(2).expect("two")),
    3_000_000,
  );
  let first = group.tasks()[0];
  let (scheduler, _events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(4).expect("slots"))
    .await
    .expect("attach");

  for (scope, hold) in [
    (HoldScope::Project, GroupHold::QueueHeld),
    (HoldScope::Group(group.key()), GroupHold::Paused),
  ] {
    assert!(db.set_queue_hold(scope, true, Timestamp::now()).expect("held"));
    scheduler.advance_task_groups().await.expect("advance");
    assert!(
      db.run_with_key(first.first_run_key()).expect("read").is_none(),
      "a held queue starts nothing"
    );
    assert_eq!(db.task_group_hold(group.key()).expect("hold"), Some(hold.clone()));
    assert_eq!(
      scheduler.start_run(first.first_run_key(), first).await,
      Err(Rejection::TaskGroupWaiting { hold }),
      "nor on request"
    );
    becomes_idle(&scheduler).await;
    assert!(db.set_queue_hold(scope, false, Timestamp::now()).expect("released"));
  }
  scheduler.advance_task_groups().await.expect("advance");
  assert!(
    db.run_with_key(first.first_run_key()).expect("read").is_some(),
    "released, the group starts"
  );
}
