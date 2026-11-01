//! Which steps of a run carry the context meter (ORC-308): an agent step whose latest execution runs or waits for the
//! person in its session. A tool step, a finished step, a step not started and an earlier execution of a step do not.

mod support;

use orchester_core::{
  id::StepExecutionId,
  run::{StepStatus, WaitReason},
  store::RunStore,
};
use orchester_workspace::tasks::detail::{RunView, StepView};
use support::{Fixture, at};

/// The fixture's run of one step, read as the task page reads it.
fn view() -> RunView {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(40));
  let run = fixture.started_run(task, at(50));
  let stored = fixture.db.run(run.id()).expect("read").expect("kept");
  orchester_workspace::tasks::detail::run_view(&fixture.db, stored).expect("view")
}

/// A step like `step`, numbered `number`, in `status` with an execution of its own unless it has none.
fn like(step: &StepView, number: usize, status: Option<StepStatus>, tool_step: bool) -> StepView {
  StepView {
    number,
    status,
    tool_step,
    execution: status.map(|_| StepExecutionId::generate()),
    ..step.clone()
  }
}

fn numbers<'a>(steps: impl Iterator<Item = &'a StepView>) -> Vec<usize> {
  steps.map(|step| step.number).collect()
}

#[test]
fn sessions_at_work_are_agent_steps_that_run_or_wait() {
  let mut view = view();
  let first = view.steps[0].clone();
  assert_eq!(first.status, Some(StepStatus::Running));
  assert_eq!(numbers(view.sessions_at_work()), [1], "the started agent step");
  view.steps = vec![
    like(&first, 1, Some(StepStatus::Passed), false),
    like(&first, 2, Some(StepStatus::Running), true),
    like(&first, 3, Some(StepStatus::Waiting(WaitReason::Message)), false),
    like(&first, 4, Some(StepStatus::Running), false),
    like(&first, 5, None, false),
    like(&first, 6, Some(StepStatus::Failed), false),
  ];
  assert_eq!(
    numbers(view.sessions_at_work()),
    [3, 4],
    "not a finished, a tool or a step not started"
  );
  let waiting = view.steps[2].execution.expect("an execution");
  assert_eq!(
    view.session_at_work(waiting).map(|step| step.number),
    Some(3),
    "a step waiting for the person"
  );
  let tool = view.steps[1].execution.expect("an execution");
  assert!(view.session_at_work(tool).is_none(), "a tool step");
  let finished = view.steps[0].execution.expect("an execution");
  assert!(view.session_at_work(finished).is_none(), "a finished step");
  assert!(
    view.session_at_work(StepExecutionId::generate()).is_none(),
    "an earlier execution of a step at work"
  );
}
