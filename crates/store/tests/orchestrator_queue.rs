//! Writing to the Orchestrator while it answers (ORC-92): a person's message kept queued, handed to the answer under
//! way or taken by it; a queued message starts its turn only once no turn of its conversation thinks and none holds a
//! message sent now, *Stop and send* first, and moves to the conversation's end then; one handed to a turn that ended,
//! or that no runner answers, waits for a turn of its own; a queued message can be removed until it starts.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, EntryKind, Queued, TurnState},
};
use orchester_store::{ProjectDb, StartedQueued};
use support::{at, model, started_run};

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("text")
}

fn keep(db: &mut ProjectDb, chat: &Chat, (text, state): (&str, &TurnState), seconds: i64) -> ChatMessageId {
  let message = ChatMessageId::generate();
  assert!(
    db.keep_chat_message_as(chat, (message, &body(text)), &[], (state, at(seconds)))
      .expect("kept")
  );
  message
}

fn turn(db: &ProjectDb, message: ChatMessageId) -> TurnState {
  db.chat_turn(message).expect("read").expect("kept").turn
}

/// The person's messages and the replies of `chat`, in the order its entries are kept.
fn said(db: &ProjectDb, chat: ChatId) -> Vec<String> {
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Person { text, .. } => Some(format!("you: {}", text.as_str())),
      EntryKind::Reply { text, .. } => Some(format!("reply: {text}")),
      _ => None,
    })
    .collect()
}

#[test]
fn a_queued_message_starts_once_the_answer_ended_and_moves_to_the_conversations_end() {
  let (_repo, mut db, _run, _execution) = started_run();
  let chat = Chat::start(ChatId::generate(), &body("Plan the release"), at(10));
  let first = keep(&mut db, &chat, ("Plan the release", &TurnState::Thinking), 10);
  db.add_chat_reply(chat.id(), ("Looking.", &model()), at(11))
    .expect("reply");
  let after = keep(
    &mut db,
    &chat,
    ("Then the notes", &TurnState::Queued(Queued::AfterAnswer)),
    12,
  );
  let next = keep(
    &mut db,
    &chat,
    ("Stop: the queue?", &TurnState::Queued(Queued::Next)),
    13,
  );
  db.add_chat_reply(chat.id(), ("Done looking.", &model()), at(14))
    .expect("reply");
  assert!(!db.queued_chat_turn_ready().expect("read"), "the answer still runs");
  assert_eq!(db.start_queued_chat_turn().expect("read"), None);
  assert!(db.end_chat_turn(first, &TurnState::Stopped, at(15)).expect("ended"));
  assert!(db.queued_chat_turn_ready().expect("read"));
  assert_eq!(
    db.start_queued_chat_turn().expect("started"),
    Some(StartedQueued {
      chat: chat.id(),
      message: next
    }),
    "Stop and send goes first"
  );
  assert_eq!(turn(&db, next), TurnState::Thinking);
  assert_eq!(turn(&db, after), TurnState::Queued(Queued::AfterAnswer));
  assert_eq!(db.start_queued_chat_turn().expect("read"), None, "one turn at a time");
  assert_eq!(
    said(&db, chat.id()),
    [
      "you: Plan the release",
      "reply: Looking.",
      "you: Then the notes",
      "reply: Done looking.",
      "you: Stop: the queue?"
    ],
    "the started message follows what the answer said before it"
  );
  assert!(db.end_chat_turn(next, &TurnState::Answered, at(16)).expect("ended"));
  assert_eq!(
    db.start_queued_chat_turn().expect("started"),
    Some(StartedQueued {
      chat: chat.id(),
      message: after
    })
  );
}

#[test]
fn a_message_handed_to_a_turn_that_ended_or_that_no_runner_answers_waits_for_its_own_turn() {
  let (_repo, mut db, _run, _execution) = started_run();
  let chat = Chat::start(ChatId::generate(), &body("Plan"), at(10));
  let first = keep(&mut db, &chat, ("Plan", &TurnState::Thinking), 10);
  let now = keep(&mut db, &chat, ("Also this", &TurnState::Steering), 11);
  assert!(!db.queued_chat_turn_ready().expect("read"));
  // The answer took it.
  assert!(
    db.change_chat_turn(now, &[TurnState::Steering], &TurnState::Joined)
      .expect("changed")
  );
  assert!(
    !db
      .change_chat_turn(now, &[TurnState::Steering], &TurnState::Queued(Queued::NotTaken))
      .expect("read"),
    "a message the answer took stays taken"
  );
  // One the turn ended without taking is left for its own turn.
  let later = keep(&mut db, &chat, ("And that", &TurnState::Steering), 12);
  assert_eq!(db.release_chat_steering(chat.id()).expect("released"), [later]);
  assert_eq!(turn(&db, later), TurnState::Queued(Queued::NotTaken));
  // A runner that attaches ends the turn no runner answers, and leaves what it held for a turn of its own.
  let held = keep(&mut db, &chat, ("One more", &TurnState::Steering), 13);
  assert_eq!(db.interrupt_chat_turns(&[], None).expect("interrupted"), [chat.id()]);
  assert!(matches!(turn(&db, first), TurnState::Failed(_)));
  assert_eq!(turn(&db, held), TurnState::Queued(Queued::NotTaken));
  assert_eq!(
    db.start_queued_chat_turn().expect("started"),
    Some(StartedQueued {
      chat: chat.id(),
      message: later
    }),
    "in the order they were kept"
  );
}

#[test]
fn a_queued_message_can_be_removed_until_its_turn_starts() {
  let (_repo, mut db, _run, _execution) = started_run();
  let chat = Chat::start(ChatId::generate(), &body("Plan"), at(10));
  let first = keep(&mut db, &chat, ("Plan", &TurnState::Thinking), 10);
  let queued = keep(
    &mut db,
    &chat,
    ("Never mind", &TurnState::Queued(Queued::AfterAnswer)),
    11,
  );
  assert!(db.remove_queued_chat_message(queued).expect("removed"));
  assert!(db.chat_turn(queued).expect("read").is_none());
  assert!(
    !db.remove_queued_chat_message(first).expect("read"),
    "a message with a turn of its own stays"
  );
  assert!(db.end_chat_turn(first, &TurnState::Answered, at(12)).expect("ended"));
  assert_eq!(db.start_queued_chat_turn().expect("read"), None);
}
