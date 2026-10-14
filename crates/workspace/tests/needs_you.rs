//! What waits for the person in a project, item by item (the Needs you panel `eBhFg`), and the count every
//! Needs you shows: a tool request, a question with suggested answers, a plan, an agent's question, a run
//! recorded interrupted, a run stopped at its limit, work ready to merge and the project's memory records
//! waiting for approval; oldest first. A run that reads interrupted only because no runner looks after it
//! is not one.

mod support;

use orchester_core::{
  id::ResourceId,
  integration::{MergeKind, RunMerge},
  memory::{
    AgentProvenance, AgentWrite, Confidence, Memory, MemoryContent, MemoryKind, MemoryScope, MemorySettings,
    MemoryStore, SecretScreen,
  },
  question::{Question, QuestionOption, QuestionRequest, QuestionTurn},
  resource::{ModelId, StepKey},
  revision::Revision,
  run::{BranchName, InterruptCause, PendingToolApproval, Run, RunLimit, WaitReason},
  store::{ActivityStore, MergeStore},
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
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
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
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
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
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
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

/// A record `run`'s agent writes to the project's memory at `now`, which waits for approval.
fn proposed(run: &Run, text: &str, now: i64) -> Memory {
  let write = AgentWrite {
    provenance: AgentProvenance {
      profile: ResourceId::generate(),
      run: run.id(),
      step: StepKey::new("implement").expect("key"),
      execution: run.current_attempt().expect("attempt").executions()[0].id(),
    },
    confidence: Confidence::Medium,
    sources: Vec::new(),
  };
  let content = MemoryContent::new(MemoryKind::Fact, text).expect("content");
  let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
  Memory::by_agent(
    MemoryScope::Project,
    write,
    screened,
    &MemorySettings::default(),
    at(now),
  )
}

#[test]
fn needs_you_lists_a_limit_stop_work_ready_to_merge_and_the_memory_waiting_as_one_item() {
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
  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
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

/// Runs `git` with `args` in `repo`, which must succeed.
fn git(repo: &std::path::Path, args: &[&str]) {
  let status = std::process::Command::new("git")
    .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
    .args(args)
    .current_dir(repo)
    .status()
    .expect("git runs");
  assert!(status.success(), "git {args:?}: {status}");
}

/// A task titled `title` whose run on `main` passed every step at `now`; with `commit`, its branch has a commit of
/// its own, else it points at main's head, as once it was merged outside the app.
fn passed(fixture: &mut Fixture, title: &str, (now, commit): (i64, bool)) -> orchester_core::id::TaskId {
  let task = fixture.task(title, at(now));
  let main = BranchName::new("main").expect("a branch");
  let mut run = fixture.started_run_into(task, Some(main), at(now + 1));
  fixture.succeed(&mut run, at(now + 2));
  let repo = fixture.repo.path().to_path_buf();
  let branch = run.branch().to_string();
  git(&repo, &["branch", &branch, "main"]);
  if commit {
    git(&repo, &["checkout", "--quiet", &branch]);
    git(&repo, &["commit", "--quiet", "--allow-empty", "-m", title]);
    git(&repo, &["checkout", "--quiet", "main"]);
  }
  task
}

#[test]
fn needs_you_leaves_out_finished_tasks_and_work_merged_outside_the_app() {
  use orchester_core::task::organization::{ChangeActor, WorkflowStatus};

  let mut fixture = Fixture::new();
  let repo = fixture.repo.path().to_path_buf();
  git(&repo, &["init", "--quiet", "--initial-branch=main"]);
  git(&repo, &["commit", "--quiet", "--allow-empty", "-m", "base"]);
  passed(&mut fixture, "Ready to merge", (10, true));
  passed(&mut fixture, "Merged by hand", (20, false));
  for (title, now, status) in [
    ("Done by hand", 30, WorkflowStatus::Done),
    ("Dropped", 40, WorkflowStatus::Canceled),
  ] {
    let task = passed(&mut fixture, title, (now, true));
    fixture
      .db
      .update_task_organization(task, ChangeActor::Person, at(now + 3), |organization| {
        organization.with_status(status)
      })
      .expect("the workflow status is saved");
  }
  let tasks = ProjectTasks::load(&repo).expect("load the project's tasks");
  let items = tasks.waiting();
  assert_eq!(kinds(&items), ["merge"], "{items:?}");
  assert_eq!(
    items[0].task.as_ref().map(|task| &*task.title),
    Some("Ready to merge"),
    "only work not on main yet, of a task not finished, waits to be merged"
  );
  assert_eq!(tasks.needs_you(), 1);
}
