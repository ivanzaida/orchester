//! What waits for the person in a project, item by item (the Needs you panel `eBhFg`), and the count every
//! Needs you shows: a tool request, a question with suggested answers, a plan, an agent's question, a run
//! recorded interrupted, a run stopped at its limit, work ready to merge and the project's memory records
//! waiting for approval; oldest first. A run that reads interrupted only because no runner looks after it
//! is not one.

mod support;

use orchester_core::{
  integration::{MergeKind, RunMerge},
  memory::MemoryStore,
  question::{Question, QuestionOption, QuestionRequest, QuestionTurn},
  resource::ModelId,
  revision::Revision,
  run::{BranchName, InterruptCause, PendingToolApproval, Run, RunLimit, WaitReason},
  store::{ActivityStore, MergeStore},
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  project_list::RunCounts,
  tasks::{
    attention::{self, Decided, NeedItem, NeedKind, SummaryCard},
    board::{BoardTab, Figures},
    data::ProjectTasks,
    list::{self, TaskRow, TaskTab},
    needs_you::{WaitingItem, WaitingKind},
  },
};
use support::{Fixture, at, proposed};

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
      WaitingKind::StoppedAtLimit { .. } => "limit",
      WaitingKind::ReadyToMerge => "merge",
      WaitingKind::Memory { .. } => "memory",
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
  let tasks = ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks");
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

/// A run of the parallel pipeline with each of its three steps started at `now`, by key.
fn parallel_run(fixture: &mut Fixture, title: &str, now: i64) -> (Run, [orchester_core::id::StepExecutionId; 3]) {
  let task = fixture.task(title, at(now));
  let mut run = fixture.started_run(task, at(now));
  let execution = |run: &Run, key: &str| {
    run
      .current_attempt()
      .expect("attempt")
      .executions()
      .iter()
      .find(|execution| execution.step().as_str() == key)
      .expect("the step runs")
      .id()
  };
  let (research, docs) = (execution(&run, "research"), execution(&run, "docs"));
  for step in [research, docs] {
    let events = run.start_step(step, at(now)).expect("the step starts beside Implement");
    fixture.record(&run, &events);
  }
  (run.clone(), [execution(&run, "implement"), research, docs])
}

#[test]
fn needs_you_lists_each_step_that_waits_in_one_run() {
  let mut fixture = Fixture::parallel();
  let (mut run, [_, research, docs]) = parallel_run(&mut fixture, "Plan zoom", 10);
  fixture.propose(&mut run, "1. Zoom.", at(20));
  let events = run
    .wait_step(research, WaitReason::Message, at(30))
    .expect("Research waits for an answer");
  fixture.record(&run, &events);
  let invocation = fixture
    .db
    .start_invocation(run.id(), docs, &ModelId::new("gpt-5.5").expect("model"), at(40))
    .expect("invocation")
    .id;
  let approval = PendingToolApproval {
    id: format!("{invocation}:call_1"),
    run: run.id(),
    execution: docs,
    invocation,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: String::new(),
    action: "cargo doc".to_owned(),
    target: None,
    requested_at: at(50),
    native: None,
  };
  fixture.db.request_tool_approval(&approval).expect("the request waits");
  let tasks = ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks");
  let items = tasks.waiting();
  let mut listed = kinds(&items);
  listed.sort_unstable();
  assert_eq!(
    listed,
    ["approval", "plan", "question"],
    "a request does not hide its run's other steps"
  );
  let steps: Vec<_> = items
    .iter()
    .filter(|item| !matches!(item.kind, WaitingKind::Approval(_)))
    .map(|item| item.step.as_ref().map(|step| step.name.to_string()))
    .collect();
  assert_eq!(steps, [Some("Implement".to_owned()), Some("Research".to_owned())]);
  assert_eq!(BoardTab::NeedsYou.rows(&tasks, 6, ", ").len(), 1, "one task waits");
  let counts = [
    BoardTab::NeedsYou.count(&tasks),
    Figures::of(&tasks, at(0)).needs_you,
    attention::items(&tasks).len(),
  ];
  assert_eq!(
    counts,
    [tasks.needs_you(); 3],
    "the tab, the figure and the cards count each waiting object, as the pill does"
  );
  assert_eq!(tasks.needs_you(), 3);
}

#[test]
fn needs_you_lists_a_step_end_question_with_options_once() {
  let mut fixture = Fixture::parallel();
  let (mut run, [_, research, docs]) = parallel_run(&mut fixture, "Place the docs button", 10);
  for (step, now) in [(research, 20), (docs, 30)] {
    let events = run
      .wait_step(step, WaitReason::Message, at(now))
      .expect("the step waits for an answer");
    fixture.record(&run, &events);
  }
  let invocation = fixture
    .db
    .start_invocation(run.id(), research, &ModelId::new("gpt-5.5").expect("model"), at(15))
    .expect("invocation")
    .id;
  let options = vec![
    QuestionOption::new("Top bar", None),
    QuestionOption::new("Sidebar", None),
  ];
  let asked = QuestionRequest::new(
    "ask-1".to_owned(),
    (run.id(), research, invocation),
    "Designer".to_owned(),
    vec![Question::new("Where should the docs button go?", None, options, false).expect("a question")],
    (at(20), QuestionTurn::StepEnd),
  )
  .expect("a request");
  fixture.db.ask_questions(&asked).expect("the question waits");
  let tasks = ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks");
  let mut listed = kinds(&tasks.waiting());
  listed.sort_unstable();
  assert_eq!(
    listed,
    ["choice", "question"],
    "Research's question is its card; Docs still waits for its own answer"
  );
}

#[test]
fn needs_you_lists_the_plans_and_questions_a_run_stopped_at_its_limit_or_interrupted_keeps() {
  let mut fixture = Fixture::parallel();
  let (mut limited, [_, research, _]) = parallel_run(&mut fixture, "Plan zoom", 10);
  fixture.propose(&mut limited, "1. Zoom.", at(20));
  let events = limited
    .wait_step(research, WaitReason::Message, at(25))
    .expect("Research waits for an answer");
  fixture.record(&limited, &events);
  let events = limited
    .stop_at_limit(RunLimit::Tokens, at(30))
    .expect("stopped at its limit");
  fixture.record(&limited, &events);
  let (mut crashed, _) = parallel_run(&mut fixture, "Fix zoom", 40);
  fixture.propose(&mut crashed, "1. Fix.", at(50));
  let events = crashed.interrupt(InterruptCause::Crashed, at(60)).expect("interrupted");
  fixture.record(&crashed, &events);
  let tasks = ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks");
  let items = tasks.waiting();
  let listed: Vec<_> = items
    .iter()
    .map(|item| {
      let task = item
        .task
        .as_ref()
        .map_or_else(String::new, |task| task.title.to_string());
      (task, kinds(std::slice::from_ref(item))[0])
    })
    .collect();
  let mut listed: Vec<_> = listed.iter().map(|(task, kind)| (task.as_str(), *kind)).collect();
  listed.sort_unstable();
  assert_eq!(
    listed,
    [
      ("Fix zoom", "interrupted"),
      ("Fix zoom", "plan"),
      ("Plan zoom", "limit"),
      ("Plan zoom", "plan"),
      ("Plan zoom", "question"),
    ],
    "the run-level stop does not hide the steps that still wait"
  );
  assert_eq!(tasks.needs_you(), 5);
}

/// A project where a run stopped at its limit, a run passed every step and waits to be merged, another's merge
/// was recorded, and two memory records wait for approval.
fn limit_merge_memory() -> ProjectTasks {
  let mut fixture = Fixture::new();
  let limited = fixture.task("Index the repo", at(10));
  let mut run = fixture.started_run(limited, at(20));
  let events = run
    .stop_at_limit(RunLimit::Tokens, at(30))
    .expect("stopped at its limit");
  fixture.record(&run, &events);
  let passed = fixture.task("Write docs", at(40));
  let mut done = fixture.started_run(passed, at(50));
  fixture.succeed(&mut done, at(60));
  let shipped = fixture.task("Add sign-out", at(61));
  let mut merged = fixture.started_run(shipped, at(62));
  fixture.succeed(&mut merged, at(63));
  let receipt = RunMerge {
    run: merged.id(),
    into: BranchName::new("main").expect("a branch"),
    revision: Revision::new(&"b".repeat(40)).expect("a revision"),
    kind: MergeKind::FastForward,
    merged_at: at(64),
  };
  fixture.db.save_run_merge(&receipt).expect("record the merge");
  let records = [
    proposed(&run, "Index with tree-sitter.", 80),
    proposed(&done, "Docs live in docs/.", 90),
  ];
  fixture
    .db
    .add_memories(&records)
    .expect("the records wait for approval");
  ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks")
}

#[test]
fn needs_you_lists_a_limit_stop_work_ready_to_merge_and_the_memory_waiting_as_one_item() {
  let tasks = limit_merge_memory();
  let items = tasks.waiting();
  assert_eq!(
    kinds(&items),
    ["limit", "merge", "memory"],
    "work whose merge was recorded waits for nothing"
  );
  assert_eq!(
    items[1].task.as_ref().map(|task| &*task.title),
    Some("Write docs"),
    "only the run that passed every step and has no merge receipt is ready to merge"
  );
  assert_eq!(items[0].kind, WaitingKind::StoppedAtLimit { daily_budget: false });
  assert_eq!(
    items[2].kind,
    WaitingKind::Memory { count: 2 },
    "the records are one item"
  );
  assert_eq!(items[2].since, at(80), "it waits since its oldest record");
  assert!(items[2].task.is_none());
  assert_eq!(tasks.needs_you(), 3);
}

#[test]
fn needs_you_items_are_decided_at_the_card_the_panel_and_the_overview_open() {
  let (_fixture, tasks) = project();
  let decided: Vec<Decided> = attention::items(&tasks).iter().map(NeedItem::decided).collect();
  assert!(
    matches!(
      decided.as_slice(),
      [
        Decided::Summary(Some(SummaryCard::Plan)),
        Decided::Conversation,
        Decided::Card(_),
        Decided::Card(_),
        Decided::Summary(None),
      ]
    ),
    "the plan, the agent's question, the request, the question with answers, the interrupted run: {decided:?}"
  );
  let decided: Vec<Decided> = attention::items(&limit_merge_memory())
    .iter()
    .map(NeedItem::decided)
    .collect();
  assert!(
    matches!(
      decided.as_slice(),
      [
        Decided::Summary(Some(SummaryCard::RaiseLimit)),
        Decided::Summary(None),
        Decided::Unavailable(_),
      ]
    ),
    "the limit, the work to merge, the memory: {decided:?}"
  );
}

#[test]
fn needs_you_counts_the_same_on_every_surface_but_the_task_list_tab() {
  let tasks = limit_merge_memory();
  assert_eq!(tasks.needs_you(), 3, "the limit stop, the work to merge and the memory");
  let counted = [
    ("Overview column", attention::count(&tasks)),
    ("Overview tab", BoardTab::NeedsYou.count(&tasks)),
    ("Need your input", Figures::of(&tasks, at(0)).needs_you),
    ("All projects", RunCounts::of(&tasks, true).needs_you),
    ("All projects offline", RunCounts::of(&tasks, false).needs_you),
  ];
  for (surface, count) in counted {
    assert_eq!(count, tasks.needs_you(), "{surface} counts what the pill counts");
  }
  let titles = |rows: Vec<TaskRow>| rows.iter().map(|row| row.title.to_string()).collect::<Vec<_>>();
  assert_eq!(
    titles(list::rows(&tasks.tasks, &tasks, TaskTab::NeedsYou)),
    ["Index the repo", "Write docs"],
    "the task list's tab lists the tasks of what waits; the memory is no task's"
  );
  assert_eq!(
    TaskTab::NeedsYou.count(&tasks),
    2,
    "the task list's tab counts what it lists"
  );
}

#[test]
fn needs_you_tab_lists_the_memory_waiting_after_its_tasks_and_counts_it() {
  let mut fixture = Fixture::new();
  let passed = fixture.task("Write docs", at(10));
  let mut done = fixture.started_run(passed, at(20));
  fixture.succeed(&mut done, at(30));
  let records = [
    proposed(&done, "Index with tree-sitter.", 40),
    proposed(&done, "Docs live in docs/.", 50),
  ];
  fixture
    .db
    .add_memories(&records)
    .expect("the records wait for approval");
  let tasks = ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks");
  assert_eq!(BoardTab::NeedsYou.rows(&tasks, 0, ", ").len(), 0);
  let lines = BoardTab::NeedsYou.project_rows(&tasks, "Kontur");
  let [line] = lines.as_slice() else {
    panic!("the records are one line: {lines:?}");
  };
  assert_eq!(line.item.kind, NeedKind::Memory);
  assert_eq!(line.item.title.key, "overview.needs.title.memoryOther");
  assert_eq!(line.detail.key, "overview.board.memoryRow");
  assert_eq!(line.detail.args, [("project", "Kontur".to_owned())]);
  assert_eq!(
    BoardTab::NeedsYou.count(&tasks),
    tasks.needs_you(),
    "the tab counts what every Needs you counts"
  );
  assert_eq!(
    BoardTab::NeedsYou.count(&tasks),
    2,
    "the task ready to merge and the records"
  );
  assert!(BoardTab::Running.project_rows(&tasks, "Kontur").is_empty());
}

#[test]
fn needs_you_is_worked_out_once_per_reading_of_the_tasks() {
  let (_fixture, tasks) = project();
  let first = tasks.waiting();
  assert!(
    std::sync::Arc::ptr_eq(&first, &tasks.waiting()),
    "asked again, the same items, not worked out again"
  );
  let copy = tasks.clone();
  assert!(
    std::sync::Arc::ptr_eq(&first, &copy.waiting()),
    "a render's copy of the reading shares them"
  );
  drop(copy);
  assert_eq!(tasks.needs_you(), first.len());
  let away = tasks.without_runner();
  assert!(
    !std::sync::Arc::ptr_eq(&first, &away.waiting()),
    "the tasks as they stand without a runner work it out for themselves"
  );
}
