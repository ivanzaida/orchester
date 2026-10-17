//! The Orchestrator chat follows its project's agents (ORC-42, `orchestrator::waits`): each wait (a question with
//! suggested answers, a step waiting for a message, a plan waiting for a decision) is kept once in the conversation,
//! linking to its card, in a conversation it opens when the project has none; with *Let the Orchestrator answer
//! agents* on, a wait starts one Orchestrator turn by itself, never while another turn of the conversation thinks
//! (it waits for that one), never for a wait the Orchestrator's own answer raised, and only while the Orchestrator has
//! what it runs on.

mod support;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog,
    SignIn},
  conversation::{AgentMessage, Message, MessageBody},
  id::{ChatId, ChatMessageId, ConnectionId, ProjectId},
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
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::waits::{Followed, follow_in};
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
  fixture.db.publish_orchestrator(&settings, Timestamp::now()).expect("published");
}

/// A run of task "Add login" at work.
fn working(fixture: &mut Fixture) -> Run {
  let task = fixture.task("Add login", at(40));
  fixture.started_run(task, at(50))
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

fn project(fixture: &Fixture) -> ProjectId {
  fixture.db.project().id()
}

fn follow(fixture: &Fixture, answers: bool) -> Followed {
  follow_in(fixture.repo.path(), project(fixture), answers).expect("followed")
}

/// The waits of the project's latest conversation, with their turns.
fn waits(fixture: &Fixture) -> Vec<(AgentWait, Option<WaitTurn>)> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  let Some(chat) = db.latest_chat().expect("read") else {
    return Vec::new();
  };
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
    (wait.kind, wait.task.as_str(), wait.number, wait.step.as_deref(), wait.agent.as_deref()),
    (WaitKind::Message, "ORC-1", 1, Some("Implement"), Some("Builder"))
  );
  assert_eq!(wait.link, format!("/project/{}/tasks/ORC-1/conversation", project(&fixture)));
  assert!(!wait.automatic && turn.is_none(), "the person's switch is off: the wait waits for them");
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
  assert!(turn.text.as_str().contains("waits for a message"), "{}", turn.text.as_str());
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
  assert!(follow(&fixture, true).turn.is_some(), "once the Orchestrator has what it runs on");
}

#[test]
fn orchestrator_waits_queue_behind_the_persons_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let chat = Chat::named(ChatId::generate(), "How is it going?", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("How is it going?").expect("text");
  fixture.db.keep_chat_message(&chat, (person, &text), at(10)).expect("kept");
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
  assert!(follow(&fixture, true).turn.is_some(), "it starts once the person's turn ended");
}

#[test]
fn orchestrator_waits_raised_by_its_own_answer_start_no_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture);
  let chat = Chat::named(ChatId::generate(), "Answer the agents", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Answer them").expect("text");
  fixture.db.keep_chat_message(&chat, (person, &text), at(10)).expect("kept");
  let sent = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Add login".to_owned(),
      change: Change::Message {
        run: run.id(),
        number: 1,
        text: "Use the email.".to_owned(),
        now: false,
      },
    })),
    ..ChatAction::new("send_message", Some("ORC-1".to_owned()), ActionOutcome::Done)
  };
  fixture.db.add_chat_action(chat.id(), &sent, at(20)).expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(30))
    .expect("ended");
  // The agent got the chat's message and asks again.
  ask(&mut fixture, &mut run);
  let followed = follow(&fixture, true);
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  let (wait, turn) = &waits(&fixture)[0];
  assert!(wait.own && !wait.automatic && turn.is_none(), "{wait:?}");
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
  let options = vec![QuestionOption::new("Top bar", None), QuestionOption::new("Sidebar", None)];
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
