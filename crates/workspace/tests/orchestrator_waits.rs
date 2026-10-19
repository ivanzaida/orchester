//! The Orchestrator chat follows its project's agents (ORC-42, `orchestrator::waits`): each wait (a question with
//! suggested answers, a step waiting for a message, a plan waiting for a decision) is kept once, linking to its card,
//! in the conversation that last dealt with its task, else in the latest one, or in a conversation it opens when the
//! project has none; with *Let the Orchestrator answer agents* on, a wait starts one Orchestrator turn by itself in
//! its conversation, never while another turn of that conversation thinks (it waits for that one), never for a wait
//! the Orchestrator's own answer raised, and only while the Orchestrator has what it runs on.

mod support;

use std::collections::HashSet;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::{AgentMessage, Arrival, Message, MessageBody, Recipient},
  id::{ChatId, ChatMessageId, ConnectionId, IdempotencyKey, InvocationId, ProjectId, StepExecutionId},
  name::Name,
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, EntryKind, OrchestratorSettings, TurnState, WaitKind, WaitTurn,
    change::{Change, ProposedChange},
  },
  question::{Question, QuestionOption, QuestionRequest, QuestionTurn},
  resource::ModelId,
  run::{Run, WaitReason},
  store::{ActivityStore, ConnectionStore, ConversationStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::waits::{Followed, MergeChecks, check_merges_in, follow_in};
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

/// A run of task "Add login" at work.
fn working(fixture: &mut Fixture) -> Run {
  let task = fixture.task("Add login", at(40));
  fixture.started_run(task, at(50))
}

/// `run`'s agent asks "Keep email sign-in?" in the conversation, and its step waits for the answer; the agent's
/// execution and invocation.
fn ask(fixture: &mut Fixture, run: &mut Run) -> (StepExecutionId, InvocationId) {
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
  (execution, invocation)
}

fn project(fixture: &Fixture) -> ProjectId {
  fixture.db.project().id()
}

/// Follows the waits with the project's runner attached.
fn follow(fixture: &Fixture, answers: bool) -> Followed {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  follow_in((root, project(fixture), &programs), &checks, (answers, true)).expect("followed")
}

/// The waits of the project's latest conversation, with their turns.
fn waits(fixture: &Fixture) -> Vec<(AgentWait, Option<WaitTurn>)> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  match db.latest_chat().expect("read") {
    Some(chat) => waits_in(fixture, chat),
    None => Vec::new(),
  }
}

/// The waits of conversation `chat`, with their turns.
fn waits_in(fixture: &Fixture, chat: ChatId) -> Vec<(AgentWait, Option<WaitTurn>)> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, turn } => Some((wait, turn)),
      _ => None,
    })
    .collect()
}

#[test]
fn orchestrator_waits_keep_one_entry_per_wait_in_a_conversation_they_open() {
  let mut fixture = Fixture::new();
  let mut run = working(&mut fixture);
  assert_eq!(follow(&fixture, false), Followed::default(), "nothing waits yet");
  ask(&mut fixture, &mut run);
  let followed = follow(&fixture, false);
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  let chats = fixture.db.chats().expect("History");
  assert_eq!(chats.len(), 1, "the wait opened a conversation");
  assert_eq!(chats[0].chat.title(), "ORC-1 · Add login");
  let kept = waits(&fixture);
  assert_eq!(kept.len(), 1);
  let (wait, turn) = &kept[0];
  assert_eq!(
    (
      wait.kind,
      wait.task.as_str(),
      wait.number,
      wait.step.as_deref(),
      wait.agent.as_deref()
    ),
    (WaitKind::Message, "ORC-1", 1, Some("Implement"), Some("Builder"))
  );
  assert_eq!(
    wait.link,
    format!("/project/{}/tasks/ORC-1/conversation", project(&fixture))
  );
  assert!(
    !wait.automatic && turn.is_none(),
    "the person's switch is off: the wait waits for them"
  );
  assert_eq!(follow(&fixture, false), Followed::default(), "kept once");
  assert_eq!(waits(&fixture).len(), 1);
}

#[test]
fn orchestrator_waits_start_one_turn_of_a_wait_while_answering_agents_is_on() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture);
  ask(&mut fixture, &mut run);
  let started = follow(&fixture, true).turn.expect("the wait starts a turn by itself");
  assert_eq!(follow(&fixture, true).turn, None, "one turn per wait");
  let kept = waits(&fixture);
  assert!(kept[0].0.automatic);
  assert_eq!(kept[0].1.as_ref().map(|turn| turn.message), Some(started.message));
  let turn = fixture.db.chat_turn(started.message).expect("read").expect("the turn");
  assert_eq!(turn.turn, TurnState::Thinking);
  assert!(
    turn.text.as_str().contains("waits for a message"),
    "{}",
    turn.text.as_str()
  );
  // The turn ends; the wait still waits, and starts no second turn.
  fixture
    .db
    .end_chat_turn(started.message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  assert_eq!(follow(&fixture, true), Followed::default());
}

#[test]
fn orchestrator_waits_start_no_turn_without_what_the_orchestrator_runs_on() {
  let mut fixture = Fixture::new();
  let mut run = working(&mut fixture);
  ask(&mut fixture, &mut run);
  let followed = follow(&fixture, true);
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  assert!(waits(&fixture)[0].0.automatic, "it would start one");
  ready(&mut fixture);
  assert!(
    follow(&fixture, true).turn.is_some(),
    "once the Orchestrator has what it runs on"
  );
}

#[test]
fn orchestrator_waits_queue_behind_the_persons_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let chat = Chat::named(ChatId::generate(), "How is it going?", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("How is it going?").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(10))
    .expect("kept");
  let mut run = working(&mut fixture);
  ask(&mut fixture, &mut run);
  let followed = follow(&fixture, true);
  assert!(followed.kept, "the wait shows in the conversation the person talks in");
  assert_eq!(followed.turn, None, "never while the person's turn thinks");
  assert_eq!(fixture.db.latest_chat().expect("read"), Some(chat.id()));
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  assert!(
    follow(&fixture, true).turn.is_some(),
    "it starts once the person's turn ended"
  );
}

/// A conversation the person asked in at `at(10)` and whose answered turn sent `run`'s agents a message.
fn answered_by_chat(fixture: &mut Fixture, run: &Run) -> Chat {
  let chat = Chat::named(ChatId::generate(), "Answer the agents", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Answer them").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(10))
    .expect("kept");
  let sent = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Add login".to_owned(),
      change: Change::Message {
        run: run.id(),
        number: 1,
        text: "Use the email.".to_owned(),
        now: false,
        then: None,
        sent_as: None,
      },
    })),
    ..ChatAction::new("send_message", Some("ORC-1".to_owned()), ActionOutcome::Done)
  };
  fixture.db.add_chat_action(chat.id(), &sent, at(20)).expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(30))
    .expect("ended");
  chat
}

#[test]
fn orchestrator_waits_raised_by_its_own_answer_start_no_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture);
  answered_by_chat(&mut fixture, &run);
  // The agent got the chat's message and asks again.
  ask(&mut fixture, &mut run);
  let followed = follow(&fixture, true);
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  let (wait, turn) = &waits(&fixture)[0];
  assert!(wait.own && !wait.automatic && turn.is_none(), "{wait:?}");
}

#[test]
fn orchestrator_waits_raised_by_its_own_answer_in_an_older_conversation_start_no_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture);
  let older = answered_by_chat(&mut fixture, &run);
  // The person starts another conversation before the agent asks again.
  let newer = Chat::named(ChatId::generate(), "Anything else?", at(40));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Anything else?").expect("text");
  fixture
    .db
    .keep_chat_message(&newer, (person, &text), at(40))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(45))
    .expect("ended");
  ask(&mut fixture, &mut run);
  let followed = follow(&fixture, true);
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  assert!(
    waits_in(&fixture, newer.id()).is_empty(),
    "the newer conversation never dealt with ORC-1"
  );
  let (wait, turn) = &waits_in(&fixture, older.id())[0];
  assert!(wait.own && !wait.automatic && turn.is_none(), "{wait:?}");
}

#[test]
fn orchestrator_waits_start_a_turn_only_once_the_runner_attached() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture);
  ask(&mut fixture, &mut run);
  let detached = follow_in(
    (fixture.repo.path(), project(&fixture), &Programs::processes()),
    &MergeChecks::default(),
    (true, false),
  )
  .expect("followed");
  assert_eq!(
    detached,
    Followed {
      kept: true,
      turn: None,
      pending: true
    },
    "the wait is kept and its turn waits for the runner"
  );
  let (wait, turn) = &waits(&fixture)[0];
  assert!(wait.automatic && turn.is_none(), "{wait:?}");
  let attached = follow(&fixture, true);
  assert!(
    !attached.kept && !attached.pending && attached.turn.is_some(),
    "{attached:?}"
  );
}

#[test]
fn orchestrator_waits_keep_a_new_entry_when_a_step_waits_for_a_message_again() {
  let mut fixture = Fixture::new();
  let mut run = working(&mut fixture);
  let (execution, invocation) = ask(&mut fixture, &mut run);
  assert!(follow(&fixture, false).kept);
  // The person answers; the agent takes it and waits again without asking anything new.
  let mut answer = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    MessageBody::new("Yes, keep it.").expect("text"),
    at(62),
  );
  fixture
    .db
    .send_message(IdempotencyKey::generate(), &answer)
    .expect("sent");
  answer
    .deliver(run.id(), (execution, invocation), at(63), Arrival::Prompt)
    .expect("delivered");
  fixture
    .db
    .save_deliveries(std::slice::from_ref(&answer))
    .expect("saved");
  let events = run.resume_step(execution, at(63)).expect("resumes");
  fixture.record(&run, &events);
  assert!(!follow(&fixture, false).kept, "the answered step waits for no one");
  let events = run.wait_step(execution, WaitReason::Message, at(66)).expect("waits");
  fixture.record(&run, &events);
  assert!(follow(&fixture, false).kept, "a second wait is a new entry");
  assert_eq!(waits(&fixture).len(), 2);
}

#[test]
fn orchestrator_waits_name_questions_with_suggested_answers_and_plans() {
  let mut fixture = Fixture::gated();
  let choosing = fixture.task("Place the docs button", at(10));
  let run = fixture.started_run(choosing, at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(21))
    .expect("invocation")
    .id;
  let options = vec![
    QuestionOption::new("Top bar", None),
    QuestionOption::new("Sidebar", None),
  ];
  let asked = QuestionRequest::new(
    "ask-1".to_owned(),
    (run.id(), execution, invocation),
    "Designer".to_owned(),
    vec![Question::new("Where should the docs button go?", None, options, false).expect("a question")],
    (at(22), QuestionTurn::InTurn),
  )
  .expect("a request");
  fixture.db.ask_questions(&asked).expect("the question waits");
  let planning = fixture.task("Plan zoom", at(30));
  let mut plan = fixture.started_run(planning, at(40));
  fixture.propose(&mut plan, "1. Zoom.", at(50));
  assert!(follow(&fixture, false).kept);
  let mut kinds: Vec<(WaitKind, String, Option<String>)> = waits(&fixture)
    .into_iter()
    .map(|(wait, _)| (wait.kind, wait.title, wait.agent))
    .collect();
  kinds.sort_by_key(|(kind, ..)| format!("{kind:?}"));
  assert_eq!(
    kinds,
    [
      (WaitKind::Plan, "Plan zoom".to_owned(), Some("Builder".to_owned())),
      (
        WaitKind::Question,
        "Place the docs button".to_owned(),
        Some("Designer".to_owned())
      ),
    ]
  );
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

/// A conversation the person talks in at `at(start)`, about nothing the agents do; its turn thinks unless `ended`.
fn talking(fixture: &mut Fixture, start: i64, ended: bool) -> Chat {
  let chat = Chat::named(ChatId::generate(), "Anything else?", at(start));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Anything else?").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(start))
    .expect("kept");
  if ended {
    fixture
      .db
      .end_chat_turn(person, &TurnState::Answered, at(start + 1))
      .expect("ended");
  }
  chat
}

/// The tasks waits of `chat` are about, as people read their keys.
fn tasks_in(fixture: &Fixture, chat: &Chat) -> Vec<String> {
  waits_in(fixture, chat.id())
    .into_iter()
    .map(|(wait, _)| wait.task)
    .collect()
}

#[test]
fn orchestrator_waits_go_to_the_conversation_that_last_dealt_with_their_task() {
  let mut fixture = Fixture::new();
  let mut login = working(&mut fixture);
  let dealt = acted_on(&mut fixture, "ORC-1", 10);
  let latest = talking(&mut fixture, 20, true);
  let zoom = fixture.task("Plan zoom", at(41));
  let mut zoom = fixture.started_run(zoom, at(51));
  ask(&mut fixture, &mut login);
  ask(&mut fixture, &mut zoom);
  assert!(follow(&fixture, false).kept);
  assert_eq!(
    tasks_in(&fixture, &dealt),
    ["ORC-1"],
    "the conversation that updated ORC-1, though another is newer"
  );
  assert_eq!(
    tasks_in(&fixture, &latest),
    ["ORC-2"],
    "no conversation dealt with ORC-2: the latest keeps it"
  );
  assert_eq!(fixture.db.chats().expect("History").len(), 2, "no conversation opened");
}

#[test]
fn orchestrator_waits_start_their_turn_in_their_conversation_while_another_thinks() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture);
  let dealt = acted_on(&mut fixture, "ORC-1", 10);
  let thinking = talking(&mut fixture, 20, false);
  ask(&mut fixture, &mut run);
  let turn = follow(&fixture, true).turn.expect("the wait starts a turn by itself");
  assert_eq!(turn.chat, dealt.id(), "in the conversation that dealt with ORC-1");
  assert_ne!(turn.chat, thinking.id());
  let (wait, started) = &waits_in(&fixture, dealt.id())[0];
  assert!(wait.automatic, "{wait:?}");
  assert_eq!(started.as_ref().map(|started| started.message), Some(turn.message));
}

#[test]
fn orchestrator_waits_raised_by_its_own_answer_start_no_turn_in_the_conversation_that_dealt_with_them_since() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture);
  let answering = answered_by_chat(&mut fixture, &run);
  // After the chat's answer, another conversation updates the task, without answering its agents.
  let since = acted_on(&mut fixture, "ORC-1", 40);
  ask(&mut fixture, &mut run);
  let followed = follow(&fixture, true);
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  assert!(
    waits_in(&fixture, answering.id()).is_empty(),
    "the newer conversation dealt with ORC-1 last"
  );
  let (wait, turn) = &waits_in(&fixture, since.id())[0];
  assert!(
    wait.own && !wait.automatic && turn.is_none(),
    "the answer in the other conversation raised it: {wait:?}"
  );
}
