//! A ready step held back only by the agent slots, as the task page's progress and the tools read what the runner
//! reports (ORC-403). What the runner reports is in `crates/runner/tests/graph.rs`, and the whole path with a real
//! runner in the app's `mcp_domain` tests.

mod support;

use std::{
  num::NonZeroU32,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  id::{RunId, StepExecutionId},
  resource::StepKey,
  run::StepStatus,
};
use orchester_ipc::{SlotWait, WorkSummary};
use orchester_workspace::{
  runner::{RunnerStatus, state::RunnerState},
  tasks::{
    detail::TaskDetail,
    progress::{ProgressState, progress, progress_with_waits},
    slot_wait::{slot_waits, waiting_steps},
  },
};

/// Both steps stand pending, and their dependencies are alike for all progress knows: only the runner's word tells a
/// step that waits for a slot from one whose dependencies have not passed.
#[test]
fn slot_wait_progress_tells_it_from_a_step_whose_dependencies_have_not_passed() {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Add login", support::at(10));
  fixture.started_run(task, support::at(20));
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("read task");
  let mut run = detail.run.expect("task has a run");
  let (ready, later) = (StepExecutionId::generate(), StepExecutionId::generate());
  for (number, key, name, execution) in [(2, "review", "Review", ready), (3, "report", "Report", later)] {
    let mut step = run.steps[0].clone();
    step.number = number;
    step.stage = number;
    step.key = StepKey::new(key).expect("valid step key");
    step.name = Arc::from(name);
    step.execution = Some(execution);
    step.status = Some(StepStatus::Pending);
    run.steps.push(step);
  }
  let states = |rows: Vec<orchester_workspace::tasks::progress::ProgressRow>| -> Vec<ProgressState> {
    rows.into_iter().skip(1).map(|row| row.state).collect()
  };
  assert_eq!(
    states(progress_with_waits(&run, false, &[ready], support::at(30))),
    [ProgressState::WaitsForSlot, ProgressState::Waiting]
  );
  assert_eq!(
    states(progress(&run, false, support::at(30))),
    [ProgressState::Waiting, ProgressState::Waiting],
    "without the runner's word no step waits for a slot"
  );
}

#[test]
fn slot_wait_names_only_the_steps_of_the_run() {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Add login", support::at(10));
  fixture.started_run(task, support::at(20));
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("read task");
  let run = detail.run.expect("task has a run");
  let implement = run.steps[0].execution.expect("the first step has an execution");
  let of_another_run = StepExecutionId::generate();
  assert_eq!(
    waiting_steps(&run.run, &[of_another_run, implement]),
    [run.steps[0].name.to_string()]
  );
  assert!(waiting_steps(&run.run, &[of_another_run]).is_empty());
}

/// Nothing stores a wait: without a runner of the project online, no step waits for a slot, even though a stopped
/// runner's status keeps what it last reported.
#[test]
fn slot_wait_is_none_while_the_runner_is_offline() {
  let root = Path::new("H:/projects/kontur");
  let execution = StepExecutionId::generate();
  let status = |project: &str, state| {
    let work = WorkSummary {
      agent_slots: NonZeroU32::MIN,
      runs: Vec::new(),
      slot_waits: vec![SlotWait {
        run: RunId::generate(),
        execution,
      }],
    };
    RunnerStatus::reported_for_test(PathBuf::from(project), state, work)
  };
  let kontur = "H:/projects/kontur";
  assert_eq!(
    slot_waits(Some(&status(kontur, RunnerState::Online)), root),
    [execution]
  );
  assert_eq!(
    slot_waits(Some(&status(kontur, RunnerState::Stopping)), root),
    [execution]
  );
  assert!(slot_waits(Some(&status(kontur, RunnerState::Stopped)), root).is_empty());
  assert!(slot_waits(Some(&status("H:/projects/other", RunnerState::Online)), root).is_empty());
  assert!(slot_waits(Some(&RunnerStatus::none()), root).is_empty());
  assert!(slot_waits(None, root).is_empty());
}
