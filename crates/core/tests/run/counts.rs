//! All projects' run counts: agents at work and runs waiting for a person, by task phase.

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
fn counts_active_and_waiting_runs_while_the_runner_is_online() {
  assert_eq!(
    RunCounts::of(PHASES, true),
    RunCounts {
      active: 2,
      needs_you: 1
    }
  );
}

#[test]
fn counts_no_active_run_without_a_runner_but_still_the_waiting_ones() {
  assert_eq!(
    RunCounts::of(PHASES, false),
    RunCounts {
      active: 0,
      needs_you: 1
    }
  );
}

#[test]
fn counts_nothing_for_a_project_without_runs() {
  assert_eq!(RunCounts::of([], true), RunCounts::default());
}
