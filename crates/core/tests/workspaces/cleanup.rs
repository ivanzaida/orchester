use orchester_core::{
  id::RunId,
  run::BranchName,
  workspaces::{CleanupChoice, Holder, Stranded, cleanup_order, default_cleanup, rescue_branch},
};

use super::{idle, limit, number};

#[test]
fn cleanup_order_takes_over_the_limit_first_then_the_longest_idle() {
  let mut held = idle(1, 0);
  held.holder = Some(Holder::Run { run: RunId::generate() });
  let mut stranded = idle(4, 0);
  stranded.stranded = Some(Stranded { run: None, files: 1 });
  let records = [held, idle(2, 30), idle(3, 10), stranded, idle(5, 50)];
  assert_eq!(cleanup_order(&records, limit(4)), [number(5), number(3), number(2)]);
}

#[test]
fn cleanup_default_leaves_unchecked_the_workspaces_of_runs_that_go_on() {
  let paused = RunId::generate();
  let mut last = idle(1, 40);
  last.last_run = Some(paused);
  let mut ended = idle(2, 10);
  ended.last_run = Some(RunId::generate());
  assert_eq!(
    default_cleanup(&[last, ended], limit(2), &[paused]),
    [
      CleanupChoice {
        number: number(2),
        checked: true
      },
      CleanupChoice {
        number: number(1),
        checked: false
      },
    ]
  );
}

#[test]
fn rescue_branch_keeps_the_run_branch_name_under_rescue() {
  let branch = BranchName::new("orchester/kon-18-run-4").expect("a branch");
  assert_eq!(rescue_branch(&branch).as_str(), "orchester/rescue/kon-18-run-4");
  let own = BranchName::new("feature/x").expect("a branch");
  assert_eq!(rescue_branch(&own).as_str(), "orchester/rescue/feature/x");
}
