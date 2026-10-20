//! Which OS notifications the app shows (ORC-56): the events read off what *Needs you* counts (each tool request,
//! question, plan, decision and limit), the latest runs' ends (merged by a person or outside, not by the
//! Orchestrator's chat; failed), the runner, the merges that stopped, the proposals and the plan limits; and the
//! notifier's rules: once per event and not for what stood at the first reading, a group's events within 30 s as one,
//! nothing while the window shows the place, in quiet hours or for a group switched off, a shared budget's stops as
//! one, an interruption and a merge held 30 s, and words that never carry a command or a message's text.

mod support;

#[path = "notifications/cases.rs"]
mod cases;
#[path = "notifications/notifier.rs"]
mod notifier;

use std::sync::Arc;

use orchester_core::{
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
  },
  conversation::{AgentMessage, Message, MessageBody},
  id::{ChatId, ProjectId},
  integration::{MergeKind, RunMerge},
  memory::MemoryStore,
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{Change, ProposedChange},
  },
  question::{Question, QuestionOption, QuestionRequest, QuestionTurn},
  resource::ModelId,
  revision::Revision,
  run::{BranchName, InterruptCause, PendingToolApproval, Run, RunLimit, StepFailure, WaitReason},
  store::{ActivityStore, ConversationStore, MergeStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  links::TaskPage,
  notifications::{
    COALESCE, Event, EventTask, Kind, LimitKind, Notification, NotificationGroup, NotificationSettings, Notifier,
    Origin, QuietHours, Scope, Target, View, WaitingProposal, merged_by_chat,
    sources::{self, MergeStop, Project, ProjectReading},
  },
  runner::state::{RunnerFailure, RunnerState},
  tasks::{attention::SummaryCard, data::ProjectTasks, list::DecisionWhy},
};
use support::{Fixture, at};

fn project() -> Project {
  Project {
    id: ProjectId::generate(),
    name: "Kontur".into(),
  }
}

fn events_of(project: &Project, tasks: &ProjectTasks) -> Vec<Event> {
  let reading = ProjectReading {
    tasks,
    runner: None,
    merges: &[],
    chat_merges: &[],
  };
  sources::of_project(project, &reading)
}

fn kinds(events: &[Event]) -> Vec<&Kind> {
  events.iter().map(|event| &event.kind).collect()
}

fn load(fixture: &Fixture) -> ProjectTasks {
  ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks")
}

fn view<'a>(now: Timestamp, focused: bool, settings: &NotificationSettings) -> View<'a> {
  View {
    now,
    minute: 12 * 60,
    focused,
    on_screen: &|_| true,
    settings: *settings,
  }
}

/// A project where a plan, a step's question, a tool request and an interrupted run wait, oldest first.
fn waiting_project() -> ProjectTasks {
  let mut fixture = Fixture::gated();
  let planning = fixture.task("Plan zoom", at(10));
  let mut plan = fixture.started_run(planning, at(20));
  fixture.propose(&mut plan, "1. Zoom.", at(30));

  let asking = fixture.task("Add login", at(40));
  let mut question = fixture.started_run(asking, at(50));
  let execution = question.current_attempt().expect("attempt").executions()[0].id();
  let events = question
    .wait_step(execution, WaitReason::Message, at(60))
    .expect("the step waits for a message");
  fixture.record(&question, &events);

  let approving = fixture.task("Add serde", at(70));
  let run = fixture.started_run(approving, at(80));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(81))
    .expect("invocation")
    .id;
  let approval = PendingToolApproval {
    id: format!("{invocation}:call_1"),
    run: run.id(),
    execution,
    invocation,
    agent: "Builder".to_owned(),
    command: true,
    scope: Some("installs serde from C:\\secret\\path".to_owned()),
    server: String::new(),
    action: "cargo add serde --token hunter2".to_owned(),
    target: None,
    requested_at: at(90),
    native: None,
  };
  fixture.db.request_tool_approval(&approval).expect("the request waits");

  let crashed = fixture.task("Fix zoom", at(130));
  let mut interrupted = fixture.started_run(crashed, at(140));
  let events = interrupted
    .interrupt(InterruptCause::Crashed, at(150))
    .expect("interrupted");
  fixture.record(&interrupted, &events);
  load(&fixture)
}

#[test]
fn notifications_read_each_needs_you_case_where_the_panel_decides_it() {
  let project = project();
  let events = events_of(&project, &waiting_project());
  let kinds: Vec<&Kind> = events.iter().map(|event| &event.kind).collect();
  assert_eq!(
    kinds,
    [
      &Kind::Plan { version: 1 },
      &Kind::Message,
      &Kind::Approval { command: true },
      &Kind::Interrupted,
    ]
  );
  let groups: Vec<NotificationGroup> = events.iter().map(Event::group).collect();
  assert_eq!(
    groups,
    [
      NotificationGroup::NeedsYou,
      NotificationGroup::NeedsYou,
      NotificationGroup::NeedsYou,
      NotificationGroup::Stopped,
    ]
  );
  let Target::Task { page, card, key, .. } = &events[0].target else {
    panic!("a plan opens its task");
  };
  assert_eq!((*page, *card), (TaskPage::Summary, Some(SummaryCard::Plan)));
  assert!(events[0].target.path().ends_with(&format!("/tasks/{key}/summary")));
  assert!(matches!(
    &events[2].target,
    Target::Task {
      page: TaskPage::Conversation,
      ..
    }
  ));
  assert_eq!(events[2].task.as_ref().map(|task| &*task.title), Some("Add serde"));
}

#[test]
fn notifications_never_carry_a_command_its_arguments_or_a_path() {
  let project = project();
  let events = events_of(&project, &waiting_project());
  let approval: Vec<(&Event, usize)> = vec![(&events[2], 0)];
  let mut notifier = Notifier::default();
  notifier.observe(Scope::Project(project.id), Vec::new(), at(0));
  notifier.observe(Scope::Project(project.id), vec![approval[0].0.clone()], at(1));
  let shown = notifier.due(&view(at(1), false, &NotificationSettings::default()));
  let words = format!("{:?}", shown);
  for secret in ["cargo", "hunter2", "secret", "serde --"] {
    assert!(!words.contains(secret), "{secret} is not shown: {words}");
  }
  assert_eq!(shown[0].title.key, "notifications.title.command");
  assert_eq!(shown[0].body.key, "notifications.body.taskStep");
}

#[test]
fn notifications_read_a_limit_a_decision_ready_and_failed_runs_but_not_memory() {
  let mut fixture = Fixture::new();
  let limited = fixture.task("Index the repo", at(10));
  let mut run = fixture.started_run(limited, at(20));
  let events = run.stop_at_limit(RunLimit::Tokens, at(30)).expect("stopped");
  fixture.record(&run, &events);

  let checking = fixture.task("Check docs", at(31));
  let mut unverified = fixture.started_run(checking, at(32));
  let execution = unverified.current_attempt().expect("attempt").executions()[0].id();
  let events = unverified
    .fail_step(execution, StepFailure::Unverified, at(33))
    .expect("waits for the person");
  fixture.record(&unverified, &events);

  let passed = fixture.task("Write docs", at(40));
  let mut done = fixture.started_run(passed, at(50));
  fixture.succeed(&mut done, at(60));

  let breaking = fixture.task("Break things", at(70));
  let mut failed = fixture.started_run(breaking, at(71));
  let execution = failed.current_attempt().expect("attempt").executions()[0].id();
  let events = failed
    .fail_step(execution, StepFailure::ReportedFailed, at(72))
    .expect("failed");
  fixture.record(&failed, &events);
  fixture
    .db
    .add_memories(&[support::proposed(&run, "Index with tree-sitter.", 80)])
    .expect("a record waits");

  let events = events_of(&project(), &load(&fixture));
  let kinds: Vec<&Kind> = events.iter().map(|event| &event.kind).collect();
  assert!(kinds.contains(&&Kind::Limit(Some(LimitKind::Tokens))), "{kinds:?}");
  assert!(kinds.contains(&&Kind::Decision(DecisionWhy::Unverified)), "{kinds:?}");
  assert!(kinds.contains(&&Kind::ReadyToMerge), "{kinds:?}");
  assert!(kinds.contains(&&Kind::Failed), "{kinds:?}");
  assert_eq!(kinds.len(), 4, "the memory records are no notification: {kinds:?}");
  let ready = events
    .iter()
    .find(|event| event.kind == Kind::ReadyToMerge)
    .expect("ready");
  assert_eq!(ready.group(), NotificationGroup::Finished);
}

#[test]
fn notifications_batch_the_runs_the_daily_budget_stopped_on_one_day() {
  let mut fixture = Fixture::new();
  for (title, now) in [("One", 10), ("Two", 20), ("Three", 30)] {
    let task = fixture.task(title, at(now));
    let mut run = fixture.started_run(task, at(now + 1));
    let events = run.stop_at_limit(RunLimit::DailyBudget, at(now + 2)).expect("stopped");
    fixture.record(&run, &events);
  }
  let project = project();
  let events = events_of(&project, &load(&fixture));
  assert_eq!(events.len(), 3);
  assert!(events[0].batch.is_some());
  assert!(events.iter().all(|event| event.batch == events[0].batch), "one batch");

  let mut notifier = Notifier::default();
  notifier.observe(Scope::Project(project.id), Vec::new(), at(0));
  notifier.observe(Scope::Project(project.id), events, at(1));
  let shown = notifier.due(&view(at(1), false, &NotificationSettings::default()));
  assert_eq!(shown.len(), 1, "one notification for the batch");
  assert_eq!(shown[0].title.key, "notifications.title.limit.dailyMany");
  assert_eq!(shown[0].title.args, [("count", "3".to_owned())]);
}

#[test]
fn notifications_read_the_runner_and_the_merges_that_stopped() {
  let tasks = waiting_project();
  let project = project();
  let ready = tasks.phases.values().find_map(|latest| latest.run()).expect("a run");
  let merges = [
    MergeStop {
      run: ready.id,
      conflicts: true,
    },
    MergeStop {
      run: ready.id,
      conflicts: false,
    },
  ];
  let not_started = RunnerState::Failed {
    cause: RunnerFailure::StartFailed,
    attempts: 0,
  };
  let reading = ProjectReading {
    tasks: &tasks,
    runner: Some(&not_started),
    merges: &merges,
    chat_merges: &[],
  };
  let events = sources::of_project(&project, &reading);
  let found = |kind: Kind| events.iter().find(|event| event.kind == kind).expect("the event");
  let conflicts = found(Kind::MergeConflicts);
  assert!(matches!(
    conflicts.target,
    Target::Task {
      page: TaskPage::Summary,
      ..
    }
  ));
  let local = found(Kind::LocalIntegration);
  assert_eq!(local.group(), NotificationGroup::Stopped);
  assert!(
    matches!(
      local.target,
      Target::Task {
        page: TaskPage::LocalIntegration,
        ..
      }
    ),
    "a merge Local integration carries on opens its page: {:?}",
    local.target
  );
  assert_ne!(conflicts.key, local.key);
  let runner = found(Kind::RunnerNotStarted);
  assert_eq!(runner.target.path(), format!("/project/{}", project.id));

  let offline = RunnerState::Failed {
    cause: RunnerFailure::Lost,
    attempts: 5,
  };
  let reconnecting = RunnerState::Reconnecting {
    attempt: 1,
    cause: RunnerFailure::Lost,
  };
  for (state, expected) in [
    (offline, Some(Kind::RunnerOffline)),
    (reconnecting, None),
    (RunnerState::Online, None),
  ] {
    let reading = ProjectReading {
      tasks: &tasks,
      runner: Some(&state),
      merges: &[],
      chat_merges: &[],
    };
    let runner = sources::of_project(&project, &reading)
      .into_iter()
      .find(|event| matches!(event.kind, Kind::RunnerOffline | Kind::RunnerNotStarted))
      .map(|event| event.kind);
    assert_eq!(runner, expected, "{state:?}");
  }
}

#[test]
fn notifications_read_proposals_in_their_own_scope() {
  let project = project();
  let chat = ChatId::generate();
  let waiting = [WaitingProposal { chat, seq: 4 }];
  let proposals = sources::of_proposals(&project, &waiting);
  assert_eq!(kinds(&proposals), [&Kind::Proposal]);
  assert_eq!(proposals[0].group(), NotificationGroup::Finished);
  assert_eq!(
    proposals[0].target.path(),
    format!("/project/{}/orchestrator?chat={chat}", project.id)
  );

  // The project was read while its conversations could not be: once they are, what waited all along is no news.
  let finished = NotificationSettings::default().with(NotificationGroup::Finished, true);
  let mut notifier = Notifier::default();
  notifier.observe(Scope::Project(project.id), Vec::new(), at(0));
  notifier.observe(Scope::Chats(project.id), proposals.clone(), at(1));
  assert!(notifier.due(&view(at(1), false, &finished)).is_empty());
  let more = sources::of_proposals(&project, &[waiting[0], WaitingProposal { chat, seq: 9 }]);
  notifier.observe(Scope::Chats(project.id), more, at(2));
  assert_eq!(notifier.due(&view(at(2), false, &finished)).len(), 1, "a new one is");
  notifier.observe(Scope::Chats(project.id), proposals, at(3));
  notifier.forget(Scope::Project(project.id));
  notifier.observe(
    Scope::Chats(project.id),
    sources::of_proposals(&project, &[WaitingProposal { chat, seq: 12 }]),
    at(100_000),
  );
  assert!(
    notifier.due(&view(at(100_000), false, &finished)).is_empty(),
    "forgetting a project forgets its conversations: their next reading is a first one"
  );
}

fn reading(percent: f64, resets: i64) -> LimitReading {
  let window = LimitWindow::new(300, Some(percent), Some(at(resets)), LimitState::Normal, false).expect("window");
  LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    account: None,
    at: at(1_000),
    limits: PlanLimits {
      windows: vec![window],
      plan: None,
      credits: None,
      blocked: None,
    },
  }
}

#[test]
fn notifications_read_a_plan_limit_at_90_percent_and_its_reset() {
  let name = |_: &LimitReading| -> Arc<str> { "Work".into() };
  let mut readings = PlanReadings::default();
  readings.observe(reading(92.0, 10_000));
  let near = sources::of_plan_limits(&readings, at(2_000), name);
  assert_eq!(near.len(), 1);
  assert_eq!(near[0].kind, Kind::PlanLimitNear { percent: 92 });
  assert_eq!(near[0].origin, Origin::Account { name: "Work".into() });
  let reset = sources::of_plan_limits(&readings, at(20_000), name);
  assert_eq!(reset[0].kind, Kind::PlanLimitReset);
  let mut low = PlanReadings::default();
  low.observe(reading(50.0, 10_000));
  assert!(sources::of_plan_limits(&low, at(2_000), name).is_empty());
}
