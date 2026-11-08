//! The waits and run ends of a task a conversation leads (ORC-761, `orchestrator::waits`): with a subscription that
//! has not ended covering the task, its waits are kept, and start their turn, in the leading conversation only (the
//! task's own subscription before its module's, the whole project's last), even when another conversation dealt
//! with the task since; a task nobody leads, or whose subscription ended before its wait was kept, keeps its waits in
//! no conversation, starts no turn and opens none (ORC-895). A tool request that came to the person is kept in the
//! leading conversation too, whatever *Let the Orchestrator answer agents* says, and starts no turn (ORC-894).
//! `cargo test -p orchester-workspace --test orchestrator_led_waits led_waits`.

mod support;

use std::collections::HashSet;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::{AgentMessage, Message, MessageBody},
  id::{ChatId, ChatMessageId, ConnectionId, ModuleId, TaskId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, EntryKind, OrchestratorSettings, RunEnd, Subscription, TurnState,
    WaitKind, WaitTurn, WatchTarget,
  },
  resource::ModelId,
  run::{DecidedBy, PendingToolApproval, Run, StepFailure, WaitReason},
  store::{ActivityStore, ConnectionStore, ConversationStore},
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::{
  chat::ChatView,
  waits::{Followed, Switches, check_merges_in, follow_in},
};
use support::{Fixture, at};

/// A checked, signed-in Claude Code connection, published as what the Orchestrator runs on.
fn ready(fixture: &mut Fixture) {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur").expect("name"),
    AgentProvider::ClaudeCodeCli,
    fixture.repo.path().join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute program");
  let haiku = CatalogModel::new(ModelId::new("haiku").expect("model"), Vec::new());
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, vec![haiku]),
      },
    })
    .expect("check");
  fixture.db.create_connection(&connection).expect("connection");
  let settings = OrchestratorSettings::new(connection.id(), ModelId::new("haiku").expect("model"), None);
  fixture
    .db
    .publish_orchestrator(&settings, Timestamp::now())
    .expect("published");
}

/// `run`'s agent asks "Keep email sign-in?" in the conversation, and its step waits for the answer.
fn ask(fixture: &mut Fixture, run: &mut Run) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(55))
    .expect("invocation")
    .id;
  let question = Message::from_agent(
    run.task(),
    run.id(),
    (execution, invocation),
    AgentMessage::Question,
    MessageBody::new("Keep email sign-in?").expect("text"),
    at(58),
  );
  fixture.db.add_agent_message(&question).expect("asked");
  let events = run.wait_step(execution, WaitReason::Message, at(60)).expect("waits");
  fixture.record(run, &events);
}

/// `run`'s one step fails.
fn fail(fixture: &mut Fixture, run: &mut Run) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run
    .fail_step(execution, StepFailure::ReportedFailed, Timestamp::now())
    .expect("failed");
  fixture.record(run, &events);
}

/// Follows the waits with the project's runner attached, *Let the Orchestrator answer agents* on when `answers`.
fn follow(fixture: &Fixture, answers: bool) -> Followed {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  let switches = Switches { drives: answers };
  follow_in((root, fixture.db.project().id(), &programs), &checks, (switches, true)).expect("followed")
}

/// The waits of conversation `chat`, with their turns.
fn waits_in(fixture: &Fixture, chat: &Chat) -> Vec<(AgentWait, Option<WaitTurn>)> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  db.chat_entries(chat.id())
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, turn } => Some((wait, turn)),
      _ => None,
    })
    .collect()
}

/// The tasks the waits of `chat` are about, as people read their keys.
fn tasks_in(fixture: &Fixture, chat: &Chat) -> Vec<String> {
  waits_in(fixture, chat).into_iter().map(|(wait, _)| wait.task).collect()
}

/// A conversation the person talked in at `at(start)`, whose answered turn did nothing to the agents.
fn talking(fixture: &mut Fixture, title: &str, start: i64) -> Chat {
  let chat = Chat::named(ChatId::generate(), title, at(start));
  let person = ChatMessageId::generate();
  let text = MessageBody::new(title).expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(start))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(start + 1))
    .expect("ended");
  chat
}

/// A conversation, started at `at(start)`, whose answered turn updated task `task` (its key).
fn acted_on(fixture: &mut Fixture, task: &str, start: i64) -> Chat {
  let chat = Chat::named(ChatId::generate(), "Tighten the brief", at(start));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Tighten the brief").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(start))
    .expect("kept");
  let updated = ChatAction::new("update_task", Some(task.to_owned()), ActionOutcome::Done);
  fixture
    .db
    .add_chat_action(chat.id(), &updated, at(start + 1))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(start + 2))
    .expect("ended");
  chat
}

/// `chat` watches `target` from `at(since)` on.
fn watch(fixture: &mut Fixture, chat: &Chat, target: WatchTarget, since: i64) {
  let subscription = Subscription::new(chat.id(), target, "", at(since));
  fixture.db.start_subscription(&subscription).expect("watching");
}

/// A module named `name`, with `tasks` in it.
fn module(fixture: &mut Fixture, name: &str, tasks: &[TaskId]) -> ModuleId {
  let details =
    ModuleDetails::new(Name::new(name).expect("name"), "", (None, None), ModuleState::Open).expect("details");
  let module = fixture.db.create_module(details, at(30)).expect("module").id();
  for &task in tasks {
    fixture
      .db
      .set_task_module(task, Some(module), ChangeActor::Person, at(31))
      .expect("moved");
  }
  module
}

#[test]
fn led_waits_go_to_the_leading_conversation_though_another_dealt_with_the_task_since() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let task = fixture.task("Add login", at(40));
  let mut run = fixture.started_run(task, at(50));
  let leading = talking(&mut fixture, "Look after ORC-1", 10);
  watch(&mut fixture, &leading, WatchTarget::Task(task), 11);
  // After the subscription, the person talks about ORC-1 in another conversation, the newer one.
  let elsewhere = acted_on(&mut fixture, "ORC-1", 20);
  ask(&mut fixture, &mut run);
  let turn = follow(&fixture, true).turn.expect("the wait starts a turn by itself");
  assert_eq!(turn.chat, leading.id(), "in the leading conversation");
  assert!(tasks_in(&fixture, &elsewhere).is_empty(), "nowhere else");
  let (wait, started) = &waits_in(&fixture, &leading)[0];
  assert!(wait.automatic, "{wait:?}");
  assert_eq!(started.as_ref().map(|started| started.message), Some(turn.message));
  assert_eq!(follow(&fixture, true), Followed::default(), "one turn per wait");
}

#[test]
fn led_waits_and_run_ends_go_to_the_task_subscription_before_the_module_one() {
  let mut fixture = Fixture::new();
  let login = fixture.task("Add login", at(40));
  let logout = fixture.task("Add logout", at(41));
  let mut login_run = fixture.started_run(login, at(50));
  let mut logout_run = fixture.started_run(logout, at(51));
  let accounts = module(&mut fixture, "Accounts", &[login, logout]);
  let by_module = talking(&mut fixture, "Look after Accounts", 10);
  let by_task = talking(&mut fixture, "Look after ORC-1", 12);
  watch(&mut fixture, &by_module, WatchTarget::Module(accounts), 13);
  watch(&mut fixture, &by_task, WatchTarget::Task(login), 14);
  ask(&mut fixture, &mut login_run);
  fail(&mut fixture, &mut logout_run);
  assert!(follow(&fixture, false).kept);
  assert_eq!(tasks_in(&fixture, &by_task), ["ORC-1"], "the task's own subscription");
  let ended = waits_in(&fixture, &by_module);
  assert_eq!(ended.len(), 1, "{ended:?}");
  assert_eq!(
    (ended[0].0.task.as_str(), ended[0].0.kind),
    ("ORC-2", WaitKind::Ended(RunEnd::Failed)),
    "the module's subscription leads the task without one of its own"
  );
}

#[test]
fn led_waits_of_a_task_nobody_leads_go_to_no_conversation_and_start_no_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let login = fixture.task("Add login", at(40));
  let zoom = fixture.task("Plan zoom", at(41));
  let avatars = fixture.task("Add avatars", at(42));
  let mut login_run = fixture.started_run(login, at(50));
  let mut zoom_run = fixture.started_run(zoom, at(51));
  let mut avatars_run = fixture.started_run(avatars, at(52));
  let watching = talking(&mut fixture, "Look after ORC-2", 5);
  watch(&mut fixture, &watching, WatchTarget::Task(zoom), 6);
  let dealt = acted_on(&mut fixture, "ORC-1", 10);
  let latest = talking(&mut fixture, "Anything else?", 20);
  ask(&mut fixture, &mut login_run);
  fail(&mut fixture, &mut zoom_run);
  ask(&mut fixture, &mut avatars_run);
  let followed = follow(&fixture, true);
  assert!(followed.kept, "{followed:?}");
  assert_eq!(
    followed.turn.map(|turn| turn.chat),
    Some(watching.id()),
    "only the led task's end starts a turn"
  );
  assert!(
    tasks_in(&fixture, &dealt).is_empty(),
    "not the conversation that dealt with ORC-1"
  );
  assert!(tasks_in(&fixture, &latest).is_empty(), "nor the latest one");
  assert_eq!(tasks_in(&fixture, &watching), ["ORC-2"], "the led task's end");
  assert_eq!(fixture.db.chats().expect("History").len(), 3, "no conversation opened");
  let turns = fixture.db.automatic_waits_without_turn().expect("read");
  assert!(turns.is_empty(), "no other wait waits for a turn: {turns:?}");
}

/// `run`'s agent asks at `at(30)` to run a command, and the request came to the person; its id.
fn request_approval(fixture: &mut Fixture, run: &Run) -> String {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(21))
    .expect("invocation")
    .id;
  let request = PendingToolApproval {
    id: format!("{invocation}:call_1"),
    run: run.id(),
    execution,
    invocation,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: "ls ~/.cargo/registry/src/".to_owned(),
    target: None,
    requested_at: at(30),
    native: None,
  };
  fixture.db.request_tool_approval(&request).expect("the request waits");
  request.id
}

#[test]
fn led_waits_keep_a_tool_request_for_the_person_in_the_leading_conversation_whatever_answer_agents_says() {
  for answers in [false, true] {
    let mut fixture = Fixture::new();
    ready(&mut fixture);
    let task = fixture.task("Add login", at(40));
    let run = fixture.started_run(task, at(50));
    let leading = talking(&mut fixture, "Look after ORC-1", 10);
    watch(&mut fixture, &leading, WatchTarget::Task(task), 11);
    let elsewhere = acted_on(&mut fixture, "ORC-1", 20);
    let request = request_approval(&mut fixture, &run);
    let followed = follow(&fixture, answers);
    assert!(followed.kept, "answer agents {answers}: {followed:?}");
    assert_eq!(followed.turn, None, "a tool request is the person's: no turn by itself");
    assert!(tasks_in(&fixture, &elsewhere).is_empty(), "nowhere else");
    let kept = waits_in(&fixture, &leading);
    assert_eq!(kept.len(), 1, "{kept:?}");
    let (wait, turn) = &kept[0];
    assert_eq!((wait.kind, wait.task.as_str()), (WaitKind::Approval, "ORC-1"));
    assert!(!wait.automatic && turn.is_none(), "{wait:?}");
    assert!(wait.link.ends_with("/conversation"), "its card: {}", wait.link);
    let shown = ChatView::load_in(fixture.repo.path(), leading.id())
      .expect("read")
      .expect("kept");
    let seq = shown
      .entries
      .iter()
      .find(|entry| matches!(entry.kind, EntryKind::Wait { .. }))
      .expect("the wait")
      .seq;
    assert!(shown.standing.contains(&seq), "it waits for the person");
    assert_eq!(follow(&fixture, answers), Followed::default(), "kept once");
    let claimed = fixture
      .db
      .claim_tool_approval(run.id(), &request, DecidedBy::Person, at(80))
      .expect("approved");
    assert!(claimed);
    let shown = ChatView::load_in(fixture.repo.path(), leading.id())
      .expect("read")
      .expect("kept");
    assert!(!shown.standing.contains(&seq), "decided: it no longer waits");
  }
}

#[test]
fn led_waits_of_tasks_no_more_specific_subscription_leads_go_to_the_whole_project() {
  let mut fixture = Fixture::new();
  let login = fixture.task("Add login", at(40));
  let zoom = fixture.task("Plan zoom", at(41));
  let avatars = fixture.task("Add avatars", at(42));
  let mut login_run = fixture.started_run(login, at(50));
  let mut zoom_run = fixture.started_run(zoom, at(51));
  let mut avatars_run = fixture.started_run(avatars, at(52));
  let accounts = module(&mut fixture, "Accounts", &[login]);
  let by_module = talking(&mut fixture, "Look after Accounts", 5);
  let by_task = talking(&mut fixture, "Look after ORC-2", 6);
  let everything = talking(&mut fixture, "Everything", 7);
  watch(&mut fixture, &by_module, WatchTarget::Module(accounts), 8);
  watch(&mut fixture, &by_task, WatchTarget::Task(zoom), 9);
  watch(&mut fixture, &everything, WatchTarget::Project, 10);
  ask(&mut fixture, &mut login_run);
  fail(&mut fixture, &mut zoom_run);
  ask(&mut fixture, &mut avatars_run);
  assert!(follow(&fixture, false).kept);
  assert_eq!(tasks_in(&fixture, &by_module), ["ORC-1"], "the module's subscription");
  assert_eq!(tasks_in(&fixture, &by_task), ["ORC-2"], "the task's own subscription");
  assert_eq!(
    tasks_in(&fixture, &everything),
    ["ORC-3"],
    "what no more specific subscription takes"
  );
}

#[test]
fn led_waits_whose_subscription_ended_before_they_were_kept_go_to_no_conversation() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let task = fixture.task("Add login", at(40));
  let mut run = fixture.started_run(task, at(50));
  let dealt = acted_on(&mut fixture, "ORC-1", 10);
  let leading = talking(&mut fixture, "Look after ORC-1", 20);
  watch(&mut fixture, &leading, WatchTarget::Task(task), 21);
  ask(&mut fixture, &mut run);
  // The subscription ends after the agent asked, before the follower keeps the wait (the app was closed).
  let stopped = fixture
    .db
    .stop_subscription(leading.id(), WatchTarget::Task(task), at(70))
    .expect("stopped");
  assert!(stopped.is_some());
  assert_eq!(follow(&fixture, true), Followed::default(), "kept nowhere, no turn");
  assert!(tasks_in(&fixture, &leading).is_empty(), "it no longer leads ORC-1");
  assert!(
    tasks_in(&fixture, &dealt).is_empty(),
    "nor does the conversation that last dealt with ORC-1"
  );
}
