//! A run whose step reported `unverified` as the task page and the overview read it: what the step could not check
//! and why, and the decision card's title.

mod support;

use orchester_core::{
  revision::Revision,
  run::{StepFailure, StepOutcome, StepOutput, Stuck, TaskPhase, UnverifiedCheck},
  store::ActivityStore,
};
use orchester_workspace::tasks::{data::ProjectTasks, detail::TaskDetail, overview::OverviewTab};

fn check() -> UnverifiedCheck {
  UnverifiedCheck {
    check: "Summary 02.B".into(),
    reason: "no fixture reaches it".into(),
  }
}

#[test]
fn unverified_run_lists_what_its_step_could_not_check() {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Add login", support::at(10));
  let mut run = fixture.started_run(task, support::at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  // The run started its step.
  let revision = Revision::new(&"b".repeat(40)).expect("revision");
  let events = run
    .fail_judged(execution, (StepFailure::Unverified, Some(revision)), support::at(31))
    .expect("unverified");
  fixture.record(&run, &events);
  let step = run.execution(execution).expect("execution").step().clone();
  fixture
    .db
    .save_step_output(&StepOutput {
      run: run.id(),
      execution,
      step: step.clone(),
      outcome: Some(StepOutcome::Unverified),
      text: "One state could not be reached.".into(),
      unverified: vec![check()],
      created_at: support::at(31),
    })
    .expect("output");

  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("detail");
  let view = detail.run.as_ref().expect("its run");
  assert!(
    matches!(view.run.status(), orchester_core::run::RunStatus::WaitingForPerson(Stuck::Unverified(waiting)) if *waiting == step)
  );
  assert_eq!(view.unverified(), [check()]);

  let tasks = ProjectTasks::load(fixture.repo.path(), &orchester_exec::programs::Programs::processes()).expect("tasks");
  assert_eq!(
    tasks
      .phases
      .values()
      .filter_map(|latest| latest.phase())
      .collect::<Vec<_>>(),
    [TaskPhase::Stuck],
    "it needs the person"
  );
  let rows = OverviewTab::NeedsYou.rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, 3);
  let decision = rows[0].decision.as_ref().expect("the decision it waits for");
  assert_eq!(decision.title().key, "runs.retryLimit.unverifiedTitle");
}
