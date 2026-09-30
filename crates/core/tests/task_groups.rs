use std::num::NonZeroU32;

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  run::{PauseReason, RunStatus},
  task_group::{GroupError, Scheduling, TaskGroup},
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
