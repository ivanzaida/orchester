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

#[test]
fn unverified_the_orchestrator_chat_may_not_accept_a_step_that_waits_unverified() {
  // Accepting a step that could not check everything is `continue_run`'s accept, which the chat's `continue_run`
  // refuses as the person's: it only proposes a retry, a return or an agent's resolution of conflicts.
  let continued = orchester_workspace::orchestrator::tools::listed()
    .into_iter()
    .find(|tool| tool.name == "continue_run")
    .and_then(|tool| tool.description.map(|told| told.into_owned()))
    .expect("the chat lists continue_run");
  assert!(
    continued.contains("raise_limits, raise_step_budget, resume and accept are the person's"),
    "{continued}"
  );
}
