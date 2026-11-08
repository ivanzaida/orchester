//! What a requested pause waits for, as the task page's header says it (`docs/design/run-pause.md`): the steps that
//! work, an agent or a command tool's program, and nothing before a pause was asked for.

mod support;

use std::sync::Arc;

use orchester_core::run::StepStatus;
use orchester_workspace::tasks::{
  detail::{RunView, TaskDetail},
  pause_wait::PauseWait,
};

fn started_run() -> RunView {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Add login", support::at(10));
  fixture.started_run(task, support::at(20));
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("read task");
  detail.run.expect("task has a run")
}

#[test]
fn pause_wait_is_empty_until_a_pause_is_asked_for() {
  let mut run = started_run();
  run.steps[0].status = Some(StepStatus::Running);
  assert!(PauseWait::of_run(&run).is_empty());
}

#[test]
fn pause_wait_names_the_agent_that_works_and_the_command_that_runs() {
  let mut run = started_run();
  run
    .run
    .request_pause(support::at(30))
    .expect("a running run takes a pause");
  run.steps[0].status = Some(StepStatus::Running);
  run.steps[0].tool_step = false;
  run.steps[0].agent = Some(Arc::from("Builder"));
  assert_eq!(
    PauseWait::of_run(&run),
    [PauseWait::Agent {
      step: run.steps[0].name.clone(),
      agent: Some(Arc::from("Builder"))
    }]
  );

  run.steps[0].tool_step = true;
  run.steps[0].agent = Some(Arc::from("cargo test"));
  assert_eq!(
    PauseWait::of_run(&run),
    [PauseWait::Command {
      step: run.steps[0].name.clone(),
      tool: Some(Arc::from("cargo test"))
    }]
  );
}

#[test]
fn pause_wait_is_empty_when_no_step_works() {
  let mut run = started_run();
  run
    .run
    .request_pause(support::at(30))
    .expect("a running run takes a pause");
  run.steps[0].status = Some(StepStatus::Waiting(orchester_core::run::WaitReason::Message));
  assert!(PauseWait::of_run(&run).is_empty());
}
