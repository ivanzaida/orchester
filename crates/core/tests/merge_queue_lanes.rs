//! The merge queue's lanes (ORC-406, `orchester_core::merge_queue::lanes`): a run is in the lane of the base branch it
//! recorded, each lane keeps the queue's order and delivers one run at a time, and what holds one lane (a run being
//! delivered, blocked, waiting for approval or gone) holds no other. `cargo test -p orchester-core --test
//! merge_queue_lanes merge_queue_lanes`.

use orchester_core::{
  id::{RunId, TaskId},
  merge_queue::{Blocked, Left, QueueEntry, QueueState, lanes},
  revision::Revision,
  run::BranchName,
  time::Timestamp,
};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("branch")
}

/// An entry at `position` of a run that recorded `base`, in `state`.
fn entry(base: Option<&str>, position: i64, state: QueueState) -> QueueEntry {
  QueueEntry {
    run: RunId::generate(),
    task: TaskId::generate(),
    base: base.map(branch),
    finished_at: Timestamp::from_unix_millis(position * 100),
    position,
    state,
    since: Timestamp::from_unix_millis(position * 100),
    resyncs: 0,
    checked: None,
    approved_at: None,
  }
}

#[test]
fn merge_queue_lanes_put_each_run_in_the_lane_of_its_recorded_base_branch_in_the_queues_order() {
  let entries = vec![
    entry(Some("main"), 1, QueueState::Queued),
    entry(Some("release/2.4"), 2, QueueState::Queued),
    entry(Some("main"), 3, QueueState::Queued),
    entry(None, 4, QueueState::Queued),
    entry(Some("release/2.4"), 5, QueueState::Queued),
  ];
  let lanes = lanes(&entries);
  let named: Vec<Option<&str>> = lanes.iter().map(|lane| lane.base.map(BranchName::as_str)).collect();
  assert_eq!(
    named,
    [Some("main"), Some("release/2.4"), None],
    "a lane per base branch, in the order of its first run"
  );
  let runs = |index: usize| -> Vec<RunId> { lanes[index].entries.iter().map(|entry| entry.run).collect() };
  assert_eq!(runs(0), [entries[0].run, entries[2].run]);
  assert_eq!(runs(1), [entries[1].run, entries[4].run]);
  assert_eq!(
    runs(2),
    [entries[3].run],
    "a run that recorded no base branch has a lane of its own"
  );
  assert!(orchester_core::merge_queue::lanes(&[]).is_empty(), "no run, no lane");
}

#[test]
fn merge_queue_lanes_deliver_one_run_each_and_a_held_lane_holds_no_other() {
  let base = Revision::new(&"a".repeat(40)).expect("revision");
  let entries = vec![
    entry(
      Some("main"),
      1,
      QueueState::Blocked(Blocked::OtherBranch {
        current: Some("feature".to_owned()),
      }),
    ),
    entry(Some("main"), 2, QueueState::Queued),
    entry(Some("release/2.4"), 3, QueueState::WaitingForApproval),
    entry(
      Some("release/2.4"),
      4,
      QueueState::Left(Left::CheckFailed {
        tool: "cargo test".to_owned(),
        detail: "failed".to_owned(),
      }),
    ),
    entry(Some("release/2.4"), 5, QueueState::Queued),
    entry(
      Some("staging"),
      6,
      QueueState::Rechecking {
        base: base.clone(),
        tip: base,
      },
    ),
  ];
  let lanes = lanes(&entries);
  let [main, release, staging] = lanes.as_slice() else {
    panic!("{lanes:?}");
  };
  assert_eq!(
    main.delivering().map(|entry| entry.run),
    Some(entries[0].run),
    "the blocked run holds main"
  );
  assert_eq!(
    release.delivering(),
    None,
    "a run waiting for approval and one that failed hold nothing"
  );
  assert_eq!(
    release.waiting().map(|entry| entry.run).collect::<Vec<_>>(),
    [entries[4].run],
    "release's waiting run moves on whatever main does"
  );
  assert_eq!(staging.delivering().map(|entry| entry.run), Some(entries[5].run));
  assert_eq!(staging.waiting().count(), 0);
}

#[test]
fn merge_queue_lanes_count_places_within_the_lane_the_run_being_delivered_first() {
  let entries = vec![
    entry(Some("main"), 1, QueueState::Queued),
    entry(Some("release/2.4"), 2, QueueState::Queued),
    entry(Some("main"), 3, QueueState::Syncing),
    entry(Some("main"), 4, QueueState::Merged(merged())),
    entry(Some("release/2.4"), 5, QueueState::Queued),
  ];
  let lanes = lanes(&entries);
  let (main, release) = (&lanes[0], &lanes[1]);
  assert_eq!(main.place(entries[2].run), Some(1), "being delivered, first");
  assert_eq!(main.place(entries[0].run), Some(2));
  assert_eq!(main.place(entries[3].run), None, "merged, out of the queue");
  assert_eq!(main.place(entries[1].run), None, "not in this lane");
  assert_eq!(release.place(entries[1].run), Some(1));
  assert_eq!(release.place(entries[4].run), Some(2));
}

fn merged() -> orchester_core::merge_queue::Delivered {
  orchester_core::merge_queue::Delivered {
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: orchester_core::integration::MergeKind::MergeCommit,
    worktree_removed: true,
    branch_deleted: true,
  }
}
