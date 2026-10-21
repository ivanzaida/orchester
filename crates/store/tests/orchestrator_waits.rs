//! The waits of the project's agents in the Orchestrator chat (ORC-42): each kept once by its key, read back as a
//! wait, and the turn a wait starts by itself, which starts only while no turn of its conversation thinks, once, and
//! reads as the wait's prompt; messages and answers the chat sent are named its, with their conversation and how they
//! went out (ORC-45); the answers and messages it proposed for a run are read back; and the migration that makes the
//! conversation's entries hold waits keeps the entries it had.
//! `cargo test -p orchester-store --test orchestrator_waits waits`.

mod support;

use orchester_core::{
  conversation::{AgentMessage, Message, MessageBody, Recipient},
  id::{ChatId, ChatMessageId, IdempotencyKey, RunId},
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, EntryKind, FromOrchestrator, SentWay, TurnFailure, TurnState, WaitKind,
    change::{Change, ProposedChange},
  },
  question::{GivenAnswer, Question, QuestionOption, QuestionRequest, QuestionTurn},
  run::Answerer,
  store::{ActivityStore, ConversationStore},
};
use orchester_store::ProjectDb;
use support::{at, copied_at_version, model, started_run};

fn wait(run: RunId) -> AgentWait {
  AgentWait {
    kind: WaitKind::Message,
    task: "ORC-1".to_owned(),
    title: "Say hello".to_owned(),
    run,
    number: 1,
    step: Some("Implement".to_owned()),
    agent: Some("Builder".to_owned()),
    place: None,
    others: 0,
    link: "/project/p/tasks/ORC-1/conversation".to_owned(),
    own: false,
    automatic: true,
    continuing: None,
    merging: None,
  }
}

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("text")
}

#[test]
fn waits_are_kept_once_and_read_back_as_waits() {
  let (_repo, mut db, run, _execution) = started_run();
  let chat = Chat::named(ChatId::generate(), "ORC-1 · Say hello", at(10));
  assert_eq!(db.latest_chat().expect("read"), None);
  assert!(
    db.keep_agent_wait(&chat, ("message:x:1", &wait(run.id())), at(10))
      .expect("kept")
  );
  assert_eq!(
    db.latest_chat().expect("read"),
    Some(chat.id()),
    "the wait opened the conversation"
  );
  let elsewhere = Chat::named(ChatId::generate(), "Another", at(11));
  assert!(
    !db
      .keep_agent_wait(&elsewhere, ("message:x:1", &wait(run.id())), at(11))
      .expect("again"),
    "a wait is kept once, whichever conversation asks"
  );
  let entries = db.chat_entries(chat.id()).expect("entries");
  assert_eq!(entries.len(), 1);
  assert_eq!(
    entries[0].kind,
    EntryKind::Wait {
      wait: wait(run.id()),
      turn: None,
    }
  );
  assert!(db.kept_wait_keys().expect("keys").contains("message:x:1"));
  let listed = db.chats().expect("History");
  assert_eq!(
    listed[0].messages, 0,
    "a wait is no message of the person's or the agent's"
  );
}

#[test]
fn waits_start_one_turn_only_while_no_turn_of_the_conversation_thinks() {
  let (_repo, mut db, run, _execution) = started_run();
  let chat = Chat::named(ChatId::generate(), "Plan", at(10));
  let person = ChatMessageId::generate();
  assert!(
    db.keep_chat_message(&chat, (person, &body("How is ORC-1?")), at(10))
      .expect("kept")
  );
  db.keep_agent_wait(&chat, ("message:x:1", &wait(run.id())), at(11))
    .expect("kept");
  let waiting = db.automatic_waits_without_turn().expect("waits");
  assert_eq!(waiting.len(), 1);
  assert_eq!(waiting[0].chat, chat.id(), "the conversation it was kept in");
  let seq = waiting[0].seq;
  let turn = ChatMessageId::generate();
  assert!(
    !db.start_wait_turn((chat.id(), seq), turn).expect("refused"),
    "the person's turn thinks: the wait waits for it"
  );
  db.end_chat_turn(person, &TurnState::Answered, at(12)).expect("ended");
  assert!(db.start_wait_turn((chat.id(), seq), turn).expect("started"));
  assert!(
    !db
      .start_wait_turn((chat.id(), seq), ChatMessageId::generate())
      .expect("not again"),
    "a wait starts one turn"
  );
  assert!(db.automatic_waits_without_turn().expect("waits").is_empty());
  let kept = db.chat_turn(turn).expect("read").expect("the turn");
  assert_eq!(kept.turn, TurnState::Thinking);
  assert!(
    kept.text.as_str().contains("waits for a message") && kept.text.as_str().contains("run #1 of ORC-1"),
    "the runner reads the wait's prompt: {}",
    kept.text.as_str()
  );
  assert!(matches!(
    &db.chat_entries(chat.id()).expect("entries")[1].kind,
    EntryKind::Wait { turn: Some(started), .. } if started.message == turn
  ));
  // A turn no runner answers ends as any turn does.
  db.interrupt_chat_turns(&[], None).expect("interrupted");
  assert_eq!(
    db.chat_turn(turn).expect("read").expect("the turn").turn,
    TurnState::Failed(TurnFailure::Interrupted)
  );
}

#[test]
fn waits_without_turn_are_the_automatic_ones_of_every_conversation_oldest_first() {
  let (_repo, mut db, run, _execution) = started_run();
  let first = Chat::named(ChatId::generate(), "First", at(10));
  let second = Chat::named(ChatId::generate(), "Second", at(11));
  db.keep_agent_wait(&second, ("message:x:1", &wait(run.id())), at(12))
    .expect("kept");
  db.keep_agent_wait(&first, ("message:x:2", &wait(run.id())), at(13))
    .expect("kept");
  let waits_for_the_person = AgentWait {
    automatic: false,
    ..wait(run.id())
  };
  db.keep_agent_wait(&first, ("message:x:3", &waits_for_the_person), at(13))
    .expect("kept");
  let waiting: Vec<(ChatId, String)> = db
    .automatic_waits_without_turn()
    .expect("waits")
    .into_iter()
    .map(|kept| (kept.chat, kept.key))
    .collect();
  assert_eq!(
    waiting,
    [
      (second.id(), "message:x:1".to_owned()),
      (first.id(), "message:x:2".to_owned())
    ]
  );
  let person = ChatMessageId::generate();
  db.keep_chat_message(&first, (person, &body("How is ORC-1?")), at(14))
    .expect("kept");
  let seq = db.automatic_waits_without_turn().expect("waits")[0].seq;
  assert!(
    db.start_wait_turn((second.id(), seq), ChatMessageId::generate())
      .expect("started"),
    "a turn thinking in another conversation does not hold it"
  );
}

#[test]
fn decision_authors_persist_person_automation_and_orchestrator_messages() {
  let (_repo, mut db, run, _) = started_run();
  let own = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    body("Keep it."),
    at(20),
  );
  let automated = own.sent_by_automation().expect("on the person's side");
  db.send_message(IdempotencyKey::generate(), &automated).expect("stored");
  let stored = db.messages(run.task()).expect("messages");
  assert_eq!(stored[0].answerer(), Some(Answerer::Automation));
  assert!(!stored[0].by_orchestrator());
}

#[test]
fn waits_name_the_messages_and_answers_the_orchestrator_sent() {
  let (_repo, mut db, run, execution) = started_run();
  let chat = ChatId::generate();
  let message =
    |text: &str, at| Message::from_person(run.task(), Some(run.id()), Recipient::Orchestrator, body(text), at);
  // Sent by the chat itself, as automation; applied by the person from its proposal; the person's own.
  let direct = message("Keep it.", at(20))
    .sent_by_orchestrator(Some(chat))
    .expect("a message on the person's side");
  let applied = message("Keep that.", at(21))
    .written_by_orchestrator(Some(chat))
    .expect("a message on the person's side");
  for sent in [direct, applied, message("Mine.", at(22))] {
    db.send_message(IdempotencyKey::generate(), &sent).expect("stored");
  }
  let stored = db.messages(run.task()).expect("messages");
  assert_eq!(
    stored.iter().map(Message::from_orchestrator).collect::<Vec<_>>(),
    [
      Some(FromOrchestrator::new(chat, SentWay::Direct)),
      Some(FromOrchestrator::new(chat, SentWay::AppliedByPerson)),
      None
    ],
    "the chat's messages keep their conversation, and how each went out is who decided it"
  );
  assert_eq!(
    stored.iter().map(Message::answerer).collect::<Vec<_>>(),
    [
      Some(Answerer::Automation),
      Some(Answerer::Person),
      Some(Answerer::Person)
    ]
  );
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(23))
    .expect("invocation");
  let reply = Message::from_agent(
    run.task(),
    run.id(),
    (execution, invocation.id),
    AgentMessage::Reply,
    body("Done."),
    at(24),
  );
  assert!(
    reply.written_by_orchestrator(Some(chat)).is_err(),
    "an agent's message is never the chat's"
  );
}

#[test]
fn waits_name_the_answers_the_orchestrator_gave() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(22))
    .expect("invocation");
  let question = Question::new(
    "Where?",
    None,
    vec![QuestionOption::new("Top", None), QuestionOption::new("Bottom", None)],
    false,
  )
  .expect("a question");
  let asked = QuestionRequest::new(
    "ask-1".to_owned(),
    (run.id(), execution, invocation.id),
    "Builder".to_owned(),
    vec![question],
    (at(24), QuestionTurn::InTurn),
  )
  .expect("a request");
  db.ask_questions(&asked).expect("asked");
  let chat = ChatId::generate();
  assert!(
    !db
      .mark_answered_by_orchestrator((run.id(), "ask-1"), chat)
      .expect("read"),
    "a question still waiting is no one's answer"
  );
  let given = asked
    .answer(&[GivenAnswer {
      choices: vec!["Top".to_owned()],
      text: None,
    }])
    .expect("answers");
  db.answer_questions((run.id(), "ask-1"), &given, (Answerer::Automation, at(25)), None)
    .expect("answered");
  assert!(
    db.mark_answered_by_orchestrator((run.id(), "ask-1"), chat)
      .expect("marked")
  );
  assert_eq!(
    db.questions(run.id()).expect("questions")[0].orchestrator,
    Some(FromOrchestrator::new(chat, SentWay::Direct)),
    "answered by automation: the chat sent it itself"
  );
  // An answer that never reached its agent is no one's, the chat's neither.
  db.undelivered_questions(run.id(), "ask-1").expect("ended");
  assert_eq!(db.questions(run.id()).expect("questions")[0].orchestrator, None);
}

#[test]
fn waits_read_the_answers_and_messages_the_orchestrator_proposed_for_a_run() {
  let (_repo, mut db, run, _execution) = started_run();
  let chat = Chat::named(ChatId::generate(), "Answers", at(10));
  db.keep_chat_message(&chat, (ChatMessageId::generate(), &body("Answer ORC-1")), at(10))
    .expect("kept");
  let change = |change: Change| {
    let mut action = ChatAction::new("send_message", Some("ORC-1".to_owned()), ActionOutcome::Waits);
    action.change = Some(Box::new(ProposedChange {
      subject: "Say hello".to_owned(),
      change,
    }));
    action
  };
  let message = Change::Message {
    run: run.id(),
    number: 1,
    text: "Keep it.".to_owned(),
    now: false,
    then: None,
    sent_as: Some("send_after_turn".to_owned()),
  };
  db.add_chat_action(chat.id(), &change(message.clone()), at(11))
    .expect("kept");
  let other_run = Change::Message {
    run: RunId::generate(),
    number: 2,
    text: "Elsewhere.".to_owned(),
    now: false,
    then: None,
    sent_as: Some("send_after_turn".to_owned()),
  };
  db.add_chat_action(chat.id(), &change(other_run), at(12)).expect("kept");
  db.add_chat_action(
    chat.id(),
    &change(Change::Cancel {
      run: run.id(),
      number: 1,
    }),
    at(13),
  )
  .expect("kept");
  let merge = Change::Merge {
    run: run.id(),
    number: 1,
    checkpoint: Some("a".repeat(40)),
    into: Some("main".to_owned()),
  };
  db.add_chat_action(chat.id(), &change(merge.clone()), at(14)).expect("kept");
  let read = db.run_answer_actions(run.id()).expect("read");
  assert_eq!(read.len(), 2, "what answers this run's agents and merges it: {read:?}");
  assert_eq!(read[0].chat, chat.id());
  assert_eq!(read[0].at, at(11));
  assert_eq!(
    read[0].action.change.as_ref().map(|change| &change.change),
    Some(&message)
  );
  assert_eq!(
    read[1].action.change.as_ref().map(|change| &change.change),
    Some(&merge),
    "the chat's merge of the run, which its task's conversation attributes (ORC-69)"
  );
}

fn schema_version(path: &std::path::Path) -> u32 {
  rusqlite::Connection::open(path)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version")
}

#[test]
fn waits_migration_keeps_the_conversations_entries() {
  let (repo, mut db, run, _execution) = started_run();
  let chat = Chat::named(ChatId::generate(), "Plan", at(10));
  let person = ChatMessageId::generate();
  db.keep_chat_message(&chat, (person, &body("Plan ORC-1")), at(10))
    .expect("kept");
  db.add_chat_reply(chat.id(), ("Planned.", &model()), at(11))
    .expect("reply");
  db.end_chat_turn(person, &TurnState::Answered, at(12)).expect("ended");
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let latest = schema_version(&current);
  let old = copied_at_version(&current, latest - 1);
  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&old.path().join(".orchester/orchester.db")), latest);
  let entries = upgraded.chat_entries(chat.id()).expect("entries");
  assert!(
    matches!(&entries[..], [first, second]
      if matches!(first.kind, EntryKind::Person { .. }) && matches!(second.kind, EntryKind::Reply { .. })),
    "{entries:?}"
  );
  assert!(
    upgraded
      .keep_agent_wait(&chat, ("message:x:1", &wait(run.id())), at(20))
      .expect("a wait after the migration")
  );
  assert_eq!(upgraded.chat_entries(chat.id()).expect("entries").len(), 3);
}
