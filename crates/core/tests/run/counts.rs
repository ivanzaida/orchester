//! All projects' run counts: agents at work by task phase, beside what waits for the person as every Needs you
//! counts it.

use orchester_core::run::{RunCounts, TaskPhase};

const PHASES: [TaskPhase; 6] = [
  TaskPhase::Running,
  TaskPhase::Running,
  TaskPhase::Waiting,
  TaskPhase::Queued,
  TaskPhase::Interrupted,
  TaskPhase::Succeeded,
];

#[test]
fn counts_active_runs_while_the_runner_is_online_beside_what_waits() {
  assert_eq!(
    RunCounts::of(PHASES, true, 3),
    RunCounts {
      active: 2,
      needs_you: 3
    }
  );
}

#[test]
fn counts_no_active_run_without_a_runner_but_still_what_waits() {
  assert_eq!(
    RunCounts::of(PHASES, false, 3),
    RunCounts {
      active: 0,
      needs_you: 3
    }
  );
}

#[test]
fn counts_nothing_for_a_project_without_runs() {
  assert_eq!(RunCounts::of([], true, 0), RunCounts::default());
}
