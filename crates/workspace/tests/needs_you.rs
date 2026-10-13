//! What waits for the person in a project, item by item (the Needs you panel `eBhFg`), and the count every
//! Needs you shows: a tool request, a question with suggested answers, a plan, an agent's question, a run
//! recorded interrupted; oldest first. A run that reads interrupted only because no runner looks after it
//! is not one.

mod support;

use orchester_core::{
  question::{Question, QuestionOption, QuestionRequest, QuestionTurn},
  resource::ModelId,
  run::{InterruptCause, PendingToolApproval, Run, WaitReason},
  store::ActivityStore,
};
use orchester_workspace::tasks::{
  data::ProjectTasks,
  needs_you::{WaitingItem, WaitingKind},
};
use support::{Fixture, at};

/// The kind of each item, in words, in the order the panel lists them.
fn kinds(items: &[WaitingItem]) -> Vec<&'static str> {
  items
    .iter()
    .map(|item| match item.kind {
      WaitingKind::Approval(_) => "approval",
      WaitingKind::Choice(_) => "choice",
      WaitingKind::Question(_) => "question",
      WaitingKind::Plan(_) => "plan",
      WaitingKind::Decision(_) => "decision",
      WaitingKind::Interrupted { .. } => "interrupted",
    })
    .collect()
}

/// `run`'s first step execution, with an invocation of its agent started at `now`.
fn invoked(
  fixture: &mut Fixture,
  run: &Run,
  now: i64,
) -> (orchester_core::id::StepExecutionId, orchester_core::id::InvocationId) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(now))
    .expect("invocation")
    .id;
  (execution, invocation)
}

/// A project where a plan, an agent's question, a tool request, a question with suggested answers and an
/// interrupted run wait, in that order of time, beside a run at work.
fn project() -> (Fixture, ProjectTasks) {
  let mut fixture = Fixture::gated();
  let planning = fixture.task("Plan zoom", at(10));
  let mut plan = fixture.started_run(planning, at(20));
  fixture.propose(&mut plan, "1. Zoom.", at(30));

  let asking = fixture.task("Add login", at(40));
  let mut question = fixture.started_run(asking, at(50));
  let execution = question.current_attempt().expect("attempt").executions()[0].id();
  let events = question
    .wait_step(execution, WaitReason::Message, at(60))
    .expect("the step waits for an answer");
  fixture.record(&question, &events);

  let approving = fixture.task("Add serde", at(70));
  let run = fixture.started_run(approving, at(80));
  let (execution, invocation) = invoked(&mut fixture, &run, 81);
  let approval = PendingToolApproval {
    id: format!("{invocation}:call_1"),
    run: run.id(),
    execution,
    invocation,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: String::new(),
    action: "cargo add serde".to_owned(),
    target: None,
    requested_at: at(90),
    native: None,
  };
  fixture.db.request_tool_approval(&approval).expect("the request waits");

  let choosing = fixture.task("Place the docs button", at(100));
  let run = fixture.started_run(choosing, at(110));
  let (execution, invocation) = invoked(&mut fixture, &run, 111);
  let options = vec![
    QuestionOption::new("Top bar", None),
    QuestionOption::new("Sidebar", None),
  ];
  let asked = QuestionRequest::new(
    "ask-1".to_owned(),
    (run.id(), execution, invocation),
    "Designer".to_owned(),
    vec![Question::new("Where should the docs button go?", None, options, false).expect("a question")],
    (at(120), QuestionTurn::InTurn),
  )
  .expect("a request");
  fixture.db.ask_questions(&asked).expect("the question waits");

  let crashed = fixture.task("Fix zoom", at(130));
  let mut interrupted = fixture.started_run(crashed, at(140));
  let events = interrupted
    .interrupt(InterruptCause::Crashed, at(150))
    .expect("interrupted");
  fixture.record(&interrupted, &events);

  let working = fixture.task("Write docs", at(160));
  fixture.started_run(working, at(170));
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
  (fixture, tasks)
}

#[test]
fn needs_you_lists_each_waiting_object_oldest_first() {
  let (_fixture, tasks) = project();
  let items = tasks.waiting();
  assert_eq!(kinds(&items), ["plan", "question", "approval", "choice", "interrupted"]);
  assert_eq!(tasks.needs_you(), 5, "every Needs you counts the panel's items");
  let titles: Vec<_> = items
    .iter()
    .map(|item| item.task.as_ref().map(|task| task.title.to_string()))
    .collect();
  assert_eq!(
    titles,
    [
      Some("Plan zoom".to_owned()),
      Some("Add login".to_owned()),
      Some("Add serde".to_owned()),
      Some("Place the docs button".to_owned()),
      Some("Fix zoom".to_owned()),
    ]
  );
  let WaitingKind::Choice(choice) = &items[3].kind else {
    panic!("a question with suggested answers");
  };
  assert_eq!(
    (&*choice.agent, &*choice.first, choice.count),
    ("Designer", "Where should the docs button go?", 1)
  );
  assert_eq!(items[2].since, at(90), "a request waits since it was asked");
}

#[test]
fn needs_you_answers_requests_and_questions_with_options_in_place_through_their_cards() {
  let (_fixture, tasks) = project();
  let keys: Vec<_> = tasks
    .waiting()
    .iter()
    .map(|item| item.kind.card_key().map(|key| key.to_string()))
    .collect();
  let request = tasks.pending_tool_approvals[0].approval.id.clone();
  assert_eq!(
    keys,
    [
      None,
      None,
      Some(format!("approval-{request}")),
      Some("question-ask-1".to_owned()),
      None,
    ]
  );
}

#[test]
fn needs_you_leaves_out_a_run_that_only_lost_its_runner() {
  let (_fixture, tasks) = project();
  let away = tasks.without_runner();
  assert_eq!(away.lost_to_runner, 1, "the run at work reads interrupted");
  assert_eq!(
    kinds(&away.waiting()),
    ["plan", "question", "approval", "choice", "interrupted"],
    "only the run recorded interrupted is listed"
  );
  assert_eq!(away.needs_you(), tasks.needs_you());
}
