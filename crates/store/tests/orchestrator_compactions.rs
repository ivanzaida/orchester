//! A person's compaction of an Orchestrator conversation's session (*Compact*, ORC-309): while it runs no queued
//! message, wait or daily report starts a turn in its conversation, and each starts once it ended; one no runner
//! carries out any more ends interrupted, but those a runner holds and those of other conversations.
//! `cargo test -p orchester-store --test orchestrator_compactions compaction`.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, RunId},
  orchestrator::{AgentWait, Chat, ChatEvent, CompactionState, EntryKind, Queued, TurnFailure, TurnState, WaitKind},
  run::Answerer,
};
use orchester_store::{ProjectDb, StartedQueued};
use support::{at, started_run};

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("text")
}

fn keep(db: &mut ProjectDb, chat: &Chat, (text, state): (&str, &TurnState), seconds: i64) -> ChatMessageId {
  let message = ChatMessageId::generate();
  assert!(
    db.keep_chat_message_as(
      chat,
      (message, &body(text)),
      (&[], Answerer::Person),
      (state, at(seconds))
    )
    .expect("kept")
  );
  message
}

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

/// The compactions of `chat`, in the order they were kept.
fn compactions(db: &ProjectDb, chat: ChatId) -> Vec<CompactionState> {
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Event(ChatEvent::Compaction { state, .. }) => Some(state),
      _ => None,
    })
    .collect()
}

#[test]
fn a_queued_message_waits_for_a_compaction_and_starts_once_it_ended() {
  let (_repo, mut db, _run, _execution) = started_run();
  let chat = Chat::start(ChatId::generate(), &body("Plan the release"), at(10));
  let first = keep(&mut db, &chat, ("Plan the release", &TurnState::Thinking), 10);
  assert!(db.end_chat_turn(first, &TurnState::Answered, at(11)).expect("ended"));
  let compaction = db
    .begin_chat_compaction(chat.id(), Some(182_000), at(12))
    .expect("begun");
  assert!(db.chat_compacting(chat.id()).expect("read"));
  let queued = keep(
    &mut db,
    &chat,
    ("Then the notes", &TurnState::Queued(Queued::AfterAnswer)),
    13,
  );
  assert!(!db.queued_chat_turn_ready().expect("read"), "the compaction runs");
  assert_eq!(db.start_queued_chat_turn().expect("read"), None);
  assert!(
    db.end_chat_compaction(compaction, CompactionState::Done { after: None }, None)
      .expect("ended")
  );
  assert!(!db.chat_compacting(chat.id()).expect("read"));
  assert!(db.queued_chat_turn_ready().expect("read"));
  assert_eq!(
    db.start_queued_chat_turn().expect("started"),
    Some(StartedQueued {
      chat: chat.id(),
      message: queued
    })
  );
}

#[test]
fn a_wait_waits_for_a_compaction_and_starts_its_turn_once_it_ended() {
  let (_repo, mut db, run, _execution) = started_run();
  let chat = Chat::named(ChatId::generate(), "ORC-1 · Say hello", at(10));
  db.keep_agent_wait(&chat, ("message:x:1", &wait(run.id())), at(10))
    .expect("kept");
  let seq = db.automatic_waits_without_turn().expect("waits")[0].seq;
  let compaction = db
    .begin_chat_compaction(chat.id(), Some(90_000), at(11))
    .expect("begun");
  let turn = ChatMessageId::generate();
  assert!(
    !db.start_wait_turn((chat.id(), seq), turn).expect("refused"),
    "the compaction runs: the wait keeps no turn and starts later"
  );
  assert_eq!(db.automatic_waits_without_turn().expect("waits").len(), 1);
  let failed = CompactionState::Failed {
    reason: TurnFailure::Interrupted,
  };
  assert!(db.end_chat_compaction(compaction, failed, None).expect("ended"));
  assert!(db.start_wait_turn((chat.id(), seq), turn).expect("started"));
  assert_eq!(
    db.chat_turn(turn).expect("read").expect("the turn").turn,
    TurnState::Thinking
  );
}

#[test]
fn a_compaction_no_runner_carries_out_ends_interrupted_but_held_ones_and_other_conversations() {
  let (_repo, mut db, _run, _execution) = started_run();
  let chat = Chat::start(ChatId::generate(), &body("Plan"), at(10));
  let other = Chat::start(ChatId::generate(), &body("Elsewhere"), at(10));
  for (chat, seconds) in [(&chat, 10), (&other, 11)] {
    let message = keep(&mut db, chat, ("Hello", &TurnState::Thinking), seconds);
    assert!(
      db.end_chat_turn(message, &TurnState::Answered, at(seconds))
        .expect("ended")
    );
  }
  let held = db.begin_chat_compaction(chat.id(), Some(1), at(12)).expect("begun");
  let elsewhere = db.begin_chat_compaction(other.id(), Some(2), at(13)).expect("begun");
  assert!(
    db.interrupt_chat_compactions(&[held], None).expect("interrupted") == [other.id()],
    "the held one runs on"
  );
  assert_eq!(compactions(&db, chat.id()), [CompactionState::Running]);
  assert_eq!(
    compactions(&db, other.id()),
    [CompactionState::Failed {
      reason: TurnFailure::Interrupted
    }]
  );
  assert!(
    !db
      .end_chat_compaction(elsewhere, CompactionState::Done { after: Some(1) }, None)
      .expect("read"),
    "one that ended stays as it ended"
  );
  assert!(
    db.interrupt_chat_compactions(&[], Some(other.id()))
      .expect("read")
      .is_empty(),
    "only the conversation named"
  );
  assert!(db.chat_compacting(chat.id()).expect("read"));
  assert_eq!(
    db.interrupt_chat_compactions(&[], Some(chat.id()))
      .expect("interrupted"),
    [chat.id()]
  );
  assert!(!db.chat_compacting(chat.id()).expect("read"));
}
