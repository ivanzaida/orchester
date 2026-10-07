//! More agent slots let a group's waiting task start at once: the runner does not wait for its next
//! pass over the groups.
use std::num::NonZeroU32;

use orchester_core::{
  run::InterruptCause,
  store::RunStore,
  task_group::{GroupHold, Scheduling},
};
use orchester_ipc::{AttachSession, Event};
use orchester_store::ProjectDb;

use super::{
  reviewed_group,
  support::{Setup, project, scheduler},
};

fn slots(count: u32) -> NonZeroU32 {
  NonZeroU32::new(count).expect("above zero")
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_waiting_for_an_agent_slot_starts_once_slots_are_raised() {
  let fixture = project(&Setup::new("Keep working."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Parallel(slots(2)), 3_000_000);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, slots(1))
    .await
    .expect("attach");
  scheduler.advance_task_groups().await.expect("advance");
  let second = || db.run_with_key(group.tasks()[1].first_run_key()).expect("read");
  assert!(second().is_none());
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::AgentCapacity)
  );

  scheduler.set_agent_slots(slots(2)).await.expect("raised");
  assert!(second().is_some(), "the waiting task starts when the slots are raised");
  assert_ne!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::AgentCapacity)
  );
  let mut announced = None;
  while let Ok(event) = events.try_recv() {
    if let Event::WorkSummary(summary) = event {
      announced = Some(summary.agent_slots);
    }
  }
  assert_eq!(announced, Some(slots(2)), "the work summary names the new slots");
  scheduler.stop_all(InterruptCause::QuitNow).await;
}
