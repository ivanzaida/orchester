//! Every turn of a conversation that leads something is told what it leads, from the store (ORC-769): a turn a wait
//! started by itself gets the conversation's subscriptions, the one its event came through and the order that
//! applies (the person's text, else the project's default order); a person's turn gets the subscriptions in its
//! standing instructions; a conversation that leads nothing keeps its prompts unchanged.
//! `cargo test -p orchester-workspace --test orchestrator_turn_leads turn_leads`.

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
    Chat, EntryKind, OrchestratorSettings, Subscription, TurnState, WatchSettings, WatchTarget,
    instructions::{ProjectReads, system_prompt, wait_prompt},
  },
  resource::ModelId,
  run::{Run, WaitReason},
  store::{ActivityStore, ConnectionStore, ConversationStore},
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::orchestrator::waits::{Switches, WaitTurn, check_merges_in, follow_in};
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

/// `run`'s agent asks a question in the conversation, and its step waits for the answer.
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

/// The turn a wait starts by itself, with *Let the Orchestrator answer agents* on and the runner attached.
fn turn(fixture: &Fixture) -> WaitTurn {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  let switches = Switches { drives: true };
  follow_in((root, fixture.db.project().id(), &programs), &checks, (switches, true))
    .expect("followed")
    .turn
    .expect("the wait starts a turn by itself")
}

/// The prompt of `turn`, as the runner reads it.
fn prompt(fixture: &Fixture, turn: WaitTurn) -> String {
  let kept = fixture.db.chat_turn(turn.message).expect("read").expect("the turn");
  kept.text.as_str().to_owned()
}

/// A conversation the person talked in at `at(start)`, whose turn was answered.
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

/// `chat` watches `target` with `order` from `at(since)` on.
fn watch(fixture: &mut Fixture, chat: &Chat, (target, order): (WatchTarget, &str), since: i64) {
  let subscription = Subscription::new(chat.id(), target, order, at(since));
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

fn default_order(fixture: &mut Fixture, order: &str) {
  let watch = WatchSettings::new(order, None).expect("settings");
  fixture.db.save_orchestrator_watch(&watch).expect("saved");
}

#[test]
fn turn_leads_of_an_event_turn_name_the_subscriptions_and_the_order_of_the_one_it_came_through() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let login = fixture.task("Add login", at(40));
  let logout = fixture.task("Add logout", at(41));
  let mut run = fixture.started_run(login, at(50));
  let accounts = module(&mut fixture, "Accounts", &[login]);
  let leading = talking(&mut fixture, "Look after Accounts", 10);
  watch(
    &mut fixture,
    &leading,
    (WatchTarget::Module(accounts), "Merge when green"),
    11,
  );
  watch(&mut fixture, &leading, (WatchTarget::Task(logout), "Only report"), 12);
  ask(&mut fixture, &mut run);
  let started = turn(&fixture);
  assert_eq!(started.chat, leading.id());
  let text = prompt(&fixture, started);
  assert!(text.starts_with("[Orchester] "), "the wait's own prompt first: {text}");
  assert!(text.contains("What this conversation leads"), "{text}");
  assert!(
    text.contains("- Accounts (a module, with the tasks added to it later), since "),
    "{text}"
  );
  assert!(text.contains("- ORC-2 (Add logout) (a task), since "), "{text}");
  assert!(
    text.contains(
      "This event came through the subscription to Accounts. The order that applies to this turn: the person's own \
       words,\n  > Merge when green"
    ),
    "{text}"
  );
}

#[test]
fn turn_leads_of_a_subscription_without_an_order_take_the_projects_default_order() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let login = fixture.task("Add login", at(40));
  let mut run = fixture.started_run(login, at(50));
  default_order(&mut fixture, "Report every failure");
  let leading = talking(&mut fixture, "Look after ORC-1", 10);
  watch(&mut fixture, &leading, (WatchTarget::Task(login), "  "), 11);
  ask(&mut fixture, &mut run);
  let text = prompt(&fixture, turn(&fixture));
  assert!(
    text.contains(
      "This event came through the subscription to ORC-1 (Add login). The order that applies to this turn: none of \
       its own, so the project's default order applies,\n  > Report every failure"
    ),
    "{text}"
  );
}

#[test]
fn turn_leads_of_a_conversation_that_leads_nothing_leave_its_prompts_unchanged() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let login = fixture.task("Add login", at(40));
  let mut run = fixture.started_run(login, at(50));
  let chat = talking(&mut fixture, "Anything else?", 10);
  ask(&mut fixture, &mut run);
  let started = turn(&fixture);
  assert_eq!(started.chat, chat.id());
  let wait = fixture
    .db
    .chat_entries(chat.id())
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Wait { wait, .. } => Some(wait),
      _ => None,
    })
    .expect("the wait");
  assert_eq!(prompt(&fixture, started), wait_prompt(&wait));
  let project = fixture.db.project().id();
  let leads = fixture.db.chat_leads(chat.id()).expect("read");
  assert!(leads.is_empty());
  assert_eq!(
    leads.system_prompt("Kontur", project, ProjectReads::FileTools),
    system_prompt("Kontur", project, ProjectReads::FileTools)
  );
}

#[test]
fn turn_leads_of_a_persons_turn_list_the_subscriptions_that_have_not_ended_with_their_orders() {
  let mut fixture = Fixture::new();
  let login = fixture.task("Add login", at(40));
  let logout = fixture.task("Add logout", at(41));
  default_order(&mut fixture, "Report every failure");
  let leading = talking(&mut fixture, "Look after the project", 10);
  let other = talking(&mut fixture, "Billing work", 12);
  watch(
    &mut fixture,
    &leading,
    (WatchTarget::Task(login), "Merge when green"),
    13,
  );
  watch(&mut fixture, &leading, (WatchTarget::WithoutModule, ""), 14);
  watch(&mut fixture, &leading, (WatchTarget::Task(logout), ""), 15);
  watch(&mut fixture, &other, (WatchTarget::Project, "Only report"), 16);
  let stopped = fixture
    .db
    .stop_subscription(leading.id(), WatchTarget::Task(logout), at(20))
    .expect("stopped");
  assert!(stopped.is_some());
  let project = fixture.db.project().id();
  let standing =
    fixture
      .db
      .chat_leads(leading.id())
      .expect("read")
      .system_prompt("Kontur", project, ProjectReads::FileTools);
  assert!(
    standing.starts_with(&system_prompt("Kontur", project, ProjectReads::FileTools)),
    "{standing}"
  );
  assert!(
    standing.contains("- ORC-1 (Add login) (a task), since 1970-01-01 00:00 UTC. Its order: the person's own words,\n  > Merge when green"),
    "{standing}"
  );
  assert!(
    standing.contains(
      "- Tasks without a module (the project's tasks without a module), since 1970-01-01 00:00 UTC. Its order: none \
       of its own, so the project's default order applies,\n  > Report every failure"
    ),
    "{standing}"
  );
  assert!(
    !standing.contains("ORC-2"),
    "an ended subscription is not listed: {standing}"
  );
  assert!(
    !standing.contains("Only report"),
    "another conversation's is not either: {standing}"
  );
}
