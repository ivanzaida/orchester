use orchester_core::{
  id::{RunId, StepExecutionId},
  workspaces::{Borrow, Holder, Stranded, WaitReason, Warmth, WorkspaceState, choose},
};

use super::{idle, limit, number};

#[test]
fn pool_borrow_takes_the_workspace_its_run_used_last() {
  let run = RunId::generate();
  let mut last = idle(2, 30);
  last.last_run = Some(run);
  let records = [idle(1, 10), last, idle(3, 20)];
  assert_eq!(choose(&records, limit(3), Holder::Run { run }), Borrow::Take(number(2)));
}

#[test]
fn pool_borrow_takes_the_one_idle_longest_when_its_last_is_busy() {
  let (run, other) = (RunId::generate(), RunId::generate());
  let mut last = idle(1, 5);
  last.last_run = Some(run);
  last.holder = Some(Holder::Run { run: other });
  let records = [last, idle(2, 40), idle(3, 20)];
  assert_eq!(choose(&records, limit(3), Holder::Run { run }), Borrow::Take(number(3)));
}

#[test]
fn pool_borrow_reuses_existing_workspaces_before_making_a_new_one() {
  let run = RunId::generate();
  let records = [idle(2, 50)];
  assert_eq!(choose(&records, limit(4), Holder::Run { run }), Borrow::Take(number(2)));
  let mut busy = idle(2, 50);
  busy.holder = Some(Holder::WarmUp);
  assert_eq!(choose(&[busy], limit(4), Holder::Run { run }), Borrow::Take(number(1)));
}

#[test]
fn pool_borrow_waits_with_how_many_are_in_use() {
  let records: Vec<_> = (1..=3)
    .map(|n| {
      let mut record = idle(n, 1);
      record.holder = Some(Holder::Run { run: RunId::generate() });
      record
    })
    .collect();
  assert_eq!(
    choose(&records, limit(3), Holder::Run { run: RunId::generate() }),
    Borrow::Wait(WaitReason { in_use: 3, of: 3 })
  );
}

#[test]
fn pool_borrow_by_a_holder_that_holds_one_gets_that_one_again() {
  let run = RunId::generate();
  let mut held = idle(3, 1);
  held.holder = Some(Holder::Run { run });
  assert_eq!(choose(&[idle(1, 0), held], limit(3), Holder::Run { run }), Borrow::Take(number(3)));
}

#[test]
fn pool_borrow_steps_beside_others_and_the_recheck_each_take_one() {
  let run = RunId::generate();
  let mut main = idle(1, 1);
  main.holder = Some(Holder::Run { run });
  main.last_run = Some(run);
  let mut step = idle(2, 2);
  step.holder = Some(Holder::Step {
    run,
    execution: StepExecutionId::generate(),
  });
  let records = [main, step];
  assert_eq!(choose(&records, limit(3), Holder::Recheck { run }), Borrow::Take(number(3)));
  assert_eq!(
    choose(&records, limit(2), Holder::Recheck { run }),
    Borrow::Wait(WaitReason { in_use: 2, of: 2 })
  );
}

#[test]
fn pool_lowering_the_limit_leaves_extra_workspaces_idle_and_over_the_limit() {
  let run = RunId::generate();
  let mut last = idle(5, 9);
  last.last_run = Some(run);
  last.warmth = Warmth::Warm;
  let records = [idle(1, 1), last.clone()];
  assert_eq!(choose(&records, limit(4), Holder::Run { run }), Borrow::Take(number(1)));
  assert_eq!(last.state(limit(4)), WorkspaceState::OverLimit);
  assert_eq!(last.state(limit(5)), WorkspaceState::Idle(Warmth::Warm));
}

#[test]
fn pool_stranded_workspace_is_never_taken_and_counts_as_in_use() {
  let run = RunId::generate();
  let mut stranded = idle(1, 0);
  let left = Stranded {
    run: Some(run),
    files: 14,
  };
  stranded.stranded = Some(left);
  stranded.last_run = Some(run);
  assert_eq!(
    choose(std::slice::from_ref(&stranded), limit(1), Holder::Run { run }),
    Borrow::Wait(WaitReason { in_use: 1, of: 1 })
  );
  assert_eq!(stranded.state(limit(1)), WorkspaceState::Stranded(left));
}

#[test]
fn pool_warm_up_shows_as_warming_up() {
  let mut warming = idle(1, 0);
  warming.holder = Some(Holder::WarmUp);
  assert_eq!(warming.state(limit(2)), WorkspaceState::WarmingUp);
}
