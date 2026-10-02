//! The status bar's activity model (`docs/design/orchester-workspace.md`, "Status bar"): the
//! activity slot's priority, failures that persist until opened or dismissed, the session's
//! recent list, and how the runner state becomes the runner item and the slot.

use std::{
  path::PathBuf,
  time::{Duration, Instant, SystemTime},
};

use orchester_desktop::{
  activity::{
    ActivityLog, OperationSpec, Outcome, Place, Slot, Target,
    bar::{ActivitySlot, RunnerBadge, activity_slot},
  },
  messages::Message,
  runner::state::{RunnerFailure, RunnerState},
};

fn spec(name: &'static str) -> OperationSpec {
  OperationSpec::new(Message::new(name), Message::new(name), Message::new(name)).target(Target::Project {
    repo_root: PathBuf::from("repo"),
    place: Place::Tasks,
  })
}

fn at(seconds: u64) -> SystemTime {
  SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

fn title(slot: &Slot<'_>) -> &'static str {
  match slot {
    Slot::Failure(event) | Slot::Latest(event) => event.title().key,
    Slot::Running { newest, .. } => newest.title().key,
    Slot::Empty => "",
  }
}

#[test]
fn activity_slot_is_empty_until_something_happens() {
  assert_eq!(ActivityLog::default().slot(), Slot::Empty);
}

#[test]
fn activity_slot_shows_the_newest_running_operation_and_the_count() {
  let mut log = ActivityLog::default();
  let now = Instant::now();
  log.start(spec("first"), now);
  log.start(spec("second"), now);
  let Slot::Running { newest, count } = log.slot() else {
    panic!("running operations take the slot");
  };
  assert_eq!((newest.title().key, count), ("second", 2));
}

#[test]
fn activity_slot_prefers_running_operations_over_the_latest_event() {
  let mut log = ActivityLog::default();
  let done = log.start(spec("done"), Instant::now());
  log.finish(done, at(1));
  assert!(matches!(log.slot(), Slot::Latest(event) if event.title().key == "done"));
  log.start(spec("running"), Instant::now());
  assert!(matches!(log.slot(), Slot::Running { count: 1, .. }));
}

#[test]
fn activity_failure_outranks_running_operations_until_dismissed() {
  let mut log = ActivityLog::default();
  let failing = log.start(spec("failing"), Instant::now());
  log.fail(failing, Message::new("reason"), at(1));
  log.start(spec("running"), Instant::now());
  assert!(matches!(log.slot(), Slot::Failure(event) if event.title().key == "failing"));

  // Later events do not replace it; it waits for the user.
  let later = log.start(spec("later"), Instant::now());
  log.finish(later, at(2));
  assert_eq!(title(&log.slot()), "failing");

  log.resolve(failing);
  assert!(matches!(log.slot(), Slot::Running { count: 1, .. }));
}

/// A run that waits for the user's decision holds the slot like a failure (02.B6 `K8VQRC`), until
/// its task is opened or it is dismissed.
#[test]
fn activity_run_waiting_for_a_decision_holds_the_slot_until_handled() {
  let mut log = ActivityLog::default();
  let target = Target::Project {
    repo_root: PathBuf::from("repo"),
    place: Place::Tasks,
  };
  log.record_needs_you(None, Message::new("waits"), target.clone(), at(1));
  log.start(spec("running"), Instant::now());
  let waiting = match log.slot() {
    Slot::Failure(event) => event.clone(),
    other => panic!("the waiting run holds the slot: {other:?}"),
  };
  assert_eq!(waiting.outcome(), &Outcome::NeedsYou { resolved: false });
  log.record(None, Message::new("later"), target, at(2));
  assert_eq!(title(&log.slot()), "waits");
  log.resolve(waiting.id());
  assert!(matches!(log.slot(), Slot::Running { count: 1, .. }));
  assert_eq!(log.recent()[1].outcome(), &Outcome::NeedsYou { resolved: true });
}

/// The waiting run's item goes once the run no longer waits, without anyone dismissing it.
#[test]
fn activity_run_waiting_for_a_decision_leaves_the_slot_once_the_run_goes_on() {
  let mut log = ActivityLog::default();
  let task = Target::Project {
    repo_root: PathBuf::from("repo"),
    place: Place::Tasks,
  };
  let other = Target::Project {
    repo_root: PathBuf::from("other"),
    place: Place::Tasks,
  };
  log.record(None, Message::new("earlier"), other.clone(), at(1));
  log.record_needs_you(None, Message::new("waits"), task.clone(), at(2));
  assert!(!log.settle_needs_you(&other), "another task's change leaves it");
  assert_eq!(title(&log.slot()), "waits");
  assert!(log.settle_needs_you(&task));
  // The stale text is gone from the slot and the list: the slot reads what happened before.
  assert_eq!(title(&log.slot()), "earlier");
  assert!(log.recent().iter().all(|event| event.title().key != "waits"));
  assert!(!log.settle_needs_you(&task), "nothing more to settle");
}

#[test]
fn activity_resolved_failure_stays_in_the_recent_list_unpinned() {
  let mut log = ActivityLog::default();
  let failing = log.start(spec("failing"), Instant::now());
  log.fail(failing, Message::new("reason"), at(1));
  let done = log.start(spec("done"), Instant::now());
  log.finish(done, at(2));

  let recent: Vec<&str> = log.recent().iter().map(|event| event.title().key).collect();
  assert_eq!(recent, ["failing", "done"], "unresolved failures are pinned first");

  log.resolve(failing);
  let recent: Vec<&str> = log.recent().iter().map(|event| event.title().key).collect();
  assert_eq!(recent, ["done", "failing"], "then newest first");
  let failure = log.recent()[1];
  assert!(!failure.is_unresolved());
  assert_eq!(
    failure.outcome(),
    &Outcome::Failed {
      reason: Message::new("reason"),
      resolved: true
    }
  );
}

#[test]
fn activity_recent_list_is_newest_first_and_folds_repeats() {
  let mut log = ActivityLog::default();
  for (name, second) in [("draft", 1), ("draft", 2), ("publish", 3), ("draft", 4)] {
    let id = log.start(spec(name), Instant::now());
    log.finish(id, at(second));
  }
  let recent: Vec<(&str, SystemTime)> = log
    .recent()
    .iter()
    .map(|event| (event.title().key, event.at()))
    .collect();
  assert_eq!(recent, [("draft", at(4)), ("publish", at(3)), ("draft", at(2))]);
}

#[test]
fn activity_cancelled_operation_leaves_no_event() {
  let mut log = ActivityLog::default();
  let id = log.start(spec("dialog"), Instant::now());
  log.cancel(id);
  assert_eq!(log.slot(), Slot::Empty);
  assert!(log.recent().is_empty());
}

#[test]
fn activity_quiet_operation_is_listed_only_once_it_finished() {
  let mut log = ActivityLog::default();
  let id = log.start(spec("autosave").quiet(), Instant::now());
  assert_eq!(log.running().count(), 0);
  assert_eq!(log.slot(), Slot::Empty);
  log.finish(id, at(1));
  assert_eq!(title(&log.slot()), "autosave");
}

#[test]
fn activity_finish_at_names_the_object_created() {
  let mut log = ActivityLog::default();
  let id = log.start(spec("create"), Instant::now());
  log.finish_at(id, Some(Target::RunnerLog), at(1));
  assert_eq!(log.recent()[0].target(), &Target::RunnerLog);
}

#[test]
fn activity_recorded_event_becomes_the_latest() {
  let mut log = ActivityLog::default();
  log.record(None, Message::new("reconnected"), Target::RunnerLog, at(1));
  assert!(matches!(log.slot(), Slot::Latest(event) if event.title().key == "reconnected"));
}

#[test]
fn runner_badge_follows_the_runner_state() {
  let failed = RunnerState::Failed {
    cause: RunnerFailure::Lost,
    attempts: 6,
  };
  let reconnecting = RunnerState::Reconnecting {
    attempt: 2,
    cause: RunnerFailure::Lost,
  };
  let cases = [
    (RunnerState::Starting, RunnerBadge::Starting),
    (RunnerState::Online, RunnerBadge::Online),
    (reconnecting, RunnerBadge::Reconnecting),
    (RunnerState::Stopping, RunnerBadge::Stopping),
    (RunnerState::Stopped, RunnerBadge::Stopped),
    (failed, RunnerBadge::Offline),
  ];
  for (state, badge) in cases {
    assert_eq!(RunnerBadge::of(&state), badge, "{state:?}");
  }
}

#[test]
fn runner_problem_outranks_the_activity_slot() {
  let mut log = ActivityLog::default();
  let failing = log.start(spec("failing"), Instant::now());
  log.fail(failing, Message::new("reason"), at(1));
  let reconnecting = RunnerState::Reconnecting {
    attempt: 1,
    cause: RunnerFailure::Lost,
  };
  let failed = RunnerState::Failed {
    cause: RunnerFailure::StartFailed,
    attempts: 0,
  };
  assert_eq!(
    activity_slot(Some(&reconnecting), &log),
    ActivitySlot::RunnerProblem(RunnerBadge::Reconnecting)
  );
  assert_eq!(
    activity_slot(Some(&failed), &log),
    ActivitySlot::RunnerProblem(RunnerBadge::Offline)
  );
}

#[test]
fn runner_starting_hides_the_activity_slot() {
  let mut log = ActivityLog::default();
  log.start(spec("running"), Instant::now());
  assert_eq!(activity_slot(Some(&RunnerState::Starting), &log), ActivitySlot::Hidden);
}

#[test]
fn runner_online_or_absent_leaves_the_slot_to_the_activity() {
  let mut log = ActivityLog::default();
  log.start(spec("running"), Instant::now());
  for runner in [Some(&RunnerState::Online), Some(&RunnerState::Stopped), None] {
    assert!(matches!(
      activity_slot(runner, &log),
      ActivitySlot::Activity(Slot::Running { count: 1, .. })
    ));
  }
}

#[test]
fn activity_work_that_did_not_happen_replaces_its_success_without_a_failure() {
  let mut log = ActivityLog::default();
  let synced = log.start(spec("synced").quiet(), Instant::now());
  log.finish(synced, at(1));
  for second in [2, 3] {
    let refused = log.start(spec("synced").quiet(), Instant::now());
    log.finish_as(refused, Message::new("notSynced"), at(second));
  }
  // The latest event says the sync did not happen; nothing is pinned, and repeats merge.
  assert_eq!(title(&log.slot()), "notSynced");
  assert!(matches!(log.slot(), Slot::Latest(event) if event.outcome() == &Outcome::NotDone));
  let recent: Vec<&str> = log.recent().iter().map(|event| event.title().key).collect();
  assert_eq!(recent, ["notSynced", "synced"]);
}

#[test]
fn activity_slot_keeps_a_run_waiting_for_a_decision_beside_approvals() {
  let mut waiting = ActivityLog::default();
  waiting.record_needs_you(
    None,
    Message::new("waits"),
    Target::Nothing,
    std::time::SystemTime::now(),
  );
  assert!(activity_slot(Some(&RunnerState::Online), &waiting).beside_approvals());
  let mut running = ActivityLog::default();
  running.start(spec("build"), Instant::now());
  assert!(!activity_slot(Some(&RunnerState::Online), &running).beside_approvals());
  assert!(!activity_slot(Some(&RunnerState::Starting), &waiting).beside_approvals());
}
