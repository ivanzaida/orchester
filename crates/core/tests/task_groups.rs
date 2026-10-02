use std::num::NonZeroU32;

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  run::{PauseReason, RunStatus},
  task_group::{GroupError, GroupHold, Scheduling, TaskGroup},
  tokens::{TokenBudget, TokensUsed},
};

fn group(mode: Scheduling) -> TaskGroup {
  TaskGroup::new(
    IdempotencyKey::generate(),
    (0..3).map(|_| TaskId::generate()).collect(),
    mode,
    TokenBudget::new(100).expect("cap"),
  )
  .expect("group")
}

#[test]
fn task_group_sequential_waits_for_review_and_holds_a_blocked_next_task() {
  let group = group(Scheduling::Sequential);
  let tasks = group.tasks();
  assert_eq!(group.eligible(&[None, None, None], TokensUsed::ZERO), vec![tasks[0]]);
  for status in [RunStatus::Running, RunStatus::Paused(PauseReason::Requested)] {
    assert!(group.eligible(&[Some(status), None, None], TokensUsed::ZERO).is_empty());
  }
  assert_eq!(
    group.eligible(&[Some(RunStatus::Succeeded), None, None], TokensUsed::ZERO),
    vec![tasks[1]]
  );
}

#[test]
fn task_group_parallel_counts_waiting_runs_and_enforces_aggregate_cap() {
  let group = group(Scheduling::Parallel(NonZeroU32::new(2).expect("positive")));
  assert_eq!(
    group.eligible(&[None, None, None], TokensUsed::ZERO),
    group.tasks()[..2]
  );
  assert_eq!(
    group.eligible(&[Some(RunStatus::Running), None, None], TokensUsed::ZERO),
    vec![group.tasks()[1]]
  );
  assert!(group.eligible(&[None, None, None], TokensUsed::new(100)).is_empty());
  assert!(
    group
      .eligible(&[Some(RunStatus::Cancelled), None, None], TokensUsed::ZERO)
      .is_empty()
  );
}

#[test]
fn task_group_rejects_empty_and_duplicate_selection() {
  let key = IdempotencyKey::generate();
  let cap = TokenBudget::new(100).expect("cap");
  let task = TaskId::generate();
  assert_eq!(
    TaskGroup::new(key, Vec::new(), Scheduling::Sequential, cap),
    Err(GroupError::Empty)
  );
  assert_eq!(
    TaskGroup::new(key, vec![task, task], Scheduling::Sequential, cap),
    Err(GroupError::Duplicate)
  );
}

#[test]
fn task_group_hold_names_why_unstarted_tasks_wait_and_who_ends_it() {
  let sequential = group(Scheduling::Sequential);
  let running = Some(RunStatus::Running);
  assert_eq!(sequential.hold(&[None, None, None], TokensUsed::ZERO), None);
  let previous = sequential.hold(&[running.clone(), None, None], TokensUsed::ZERO);
  assert_eq!(previous, Some(GroupHold::PreviousTask));
  assert!(previous.is_some_and(|hold| hold.clears_on_its_own()));
  let parallel = group(Scheduling::Parallel(NonZeroU32::new(1).expect("positive")));
  assert_eq!(
    parallel.hold(&[running.clone(), None, None], TokensUsed::ZERO),
    Some(GroupHold::Concurrency)
  );
  let failed = Some(RunStatus::Cancelled);
  let held = sequential.hold(&[failed.clone(), None, None], TokensUsed::ZERO);
  assert_eq!(held, Some(GroupHold::FailedTask));
  assert!(held.is_some_and(|hold| !hold.clears_on_its_own() && hold.releases_unstarted()));
  let cap = sequential.hold(&[running.clone(), None, None], TokensUsed::new(100));
  assert_eq!(cap, Some(GroupHold::GroupCap));
  assert!(cap.is_some_and(|hold| hold.releases_unstarted()));
  assert!(!GroupHold::SessionEnded.clears_on_its_own() && !GroupHold::SessionEnded.releases_unstarted());
  // Only a failed task and the spent cap outlive a session; a start to fix waits for Start run.
  assert!(GroupHold::FailedTask.outlives_session() && GroupHold::GroupCap.outlives_session());
  for hold in [
    GroupHold::StartFailed,
    GroupHold::Preflight(Vec::new()),
    GroupHold::PreviousTask,
  ] {
    assert!(!hold.outlives_session());
  }
  // Nothing is left to start, and nothing holds the group: no hold.
  let done = Some(RunStatus::Succeeded);
  assert_eq!(
    sequential.hold(&[done.clone(), done.clone(), running], TokensUsed::ZERO),
    None
  );
  assert!(!TaskGroup::finished(&[done.clone(), done.clone(), None]));
  assert!(TaskGroup::finished(&[done.clone(), failed, done]));
}

#[test]
fn task_group_work_of_its_own_waits_only_on_moving_members() {
  let sequential = group(Scheduling::Sequential);
  assert!(sequential.has_work_of_its_own(&[None, None, None], TokensUsed::ZERO));
  assert!(sequential.has_work_of_its_own(&[Some(RunStatus::Running), None, None], TokensUsed::ZERO));
  assert!(sequential.has_work_of_its_own(&[Some(RunStatus::Pending), None, None], TokensUsed::ZERO));
  for stuck in [
    RunStatus::Paused(PauseReason::Requested),
    RunStatus::Interrupted(orchester_core::run::InterruptCause::RunnerLost),
    RunStatus::Cancelled,
  ] {
    assert!(!sequential.has_work_of_its_own(&[Some(stuck), None, None], TokensUsed::ZERO));
  }
  assert!(!sequential.has_work_of_its_own(&[Some(RunStatus::Running), None, None], TokensUsed::new(100)));
  let done = Some(RunStatus::Succeeded);
  assert!(!sequential.has_work_of_its_own(&[done.clone(), done.clone(), done], TokensUsed::ZERO));
}
