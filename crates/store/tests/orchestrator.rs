//! The Orchestrator chat's records: its settings, a person's message kept once under its id and forgotten when the
//! runner never took it, how its turn ends (once), the session, replies and actions in order, and History's list.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId},
  orchestrator::{
    ActionOutcome, Chat, ChatAction, ChatEvent, EntryKind, OrchestratorSettings, SessionChange, TurnFailure, TurnState,
  },
};
use support::{at, model, started_run};

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("text")
}

#[test]
fn orchestrator_settings_are_none_until_saved_then_replaced() {
  let (_repo, mut db, _run, _execution) = started_run();
  assert_eq!(db.orchestrator_settings().expect("read"), None);
  let first = OrchestratorSettings::new(ConnectionId::generate(), model(), Some("high".to_owned()));
  db.save_orchestrator_settings(&first).expect("saved");
  let second = OrchestratorSettings::new(ConnectionId::generate(), model(), None);
  db.save_orchestrator_settings(&second).expect("replaced");
  assert_eq!(db.orchestrator_settings().expect("read"), Some(second));
}

#[test]
fn a_chat_message_is_kept_once_and_forgotten_with_the_chat_it_alone_started() {
  let (_repo, mut db, _run, _execution) = started_run();
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let started = Chat::start(chat, &body("Plan the webhook work\nwith tasks"), at(10));
  assert_eq!(started.title(), "Plan the webhook work");
  assert!(
    db.keep_chat_message(&started, (message, &body("Plan the webhook work")), at(10))
      .expect("kept")
  );
  assert!(
    !db
      .keep_chat_message(&started, (message, &body("Plan the webhook work")), at(20))
      .expect("again"),
    "sent again, it is kept once"
  );
  assert_eq!(db.chat_entries(chat).expect("entries").len(), 1);
  db.forget_chat_message(message).expect("forgotten");
  assert_eq!(db.chat(chat).expect("read"), None, "the chat it started went with it");
  assert_eq!(db.chat_turn(message).expect("read"), None);
}

#[test]
fn a_turn_ends_once_and_its_chat_keeps_replies_actions_and_session_in_order() {
  let (_repo, mut db, _run, _execution) = started_run();
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let started = Chat::start(chat, &body("What runs?"), at(10));
  db.keep_chat_message(&started, (message, &body("What runs?")), at(10))
    .expect("kept");
  let (connection, account) = (ConnectionId::generate(), AgentAccountId::generate());
  db.set_chat_session(chat, "session-1", (connection, account))
    .expect("session");
  db.add_chat_reply(chat, ("Two runs.", &model()), at(20)).expect("reply");
  let action = ChatAction {
    tool: "run_task".to_owned(),
    task: Some("ORC-2".to_owned()),
    outcome: ActionOutcome::Refused {
      code: "task_blocked".to_owned(),
      reason: "ORC-1 must be merged first.".to_owned(),
    },
  };
  db.add_chat_action(chat, &action, at(30)).expect("action");
  assert!(db.end_chat_turn(message, &TurnState::Answered).expect("ended"));
  assert!(
    !db.end_chat_turn(message, &TurnState::Stopped).expect("read"),
    "a turn ends once"
  );

  let kept = db.chat(chat).expect("read").expect("kept");
  assert_eq!(kept.resumes_on(connection, account), Some("session-1"));
  assert_eq!(kept.session_change(connection, account), None);
  assert_eq!(
    kept.resumes_on(ConnectionId::generate(), account),
    None,
    "another connection starts a session"
  );
  let elsewhere = AgentAccountId::generate();
  assert_eq!(
    kept.resumes_on(connection, elsewhere),
    None,
    "the connection on another account starts a session: the old one is in another CLI folder"
  );
  assert_eq!(kept.session_change(connection, elsewhere), Some(SessionChange::Account));
  db.add_chat_event(
    chat,
    &ChatEvent::NewSession {
      because: SessionChange::Account,
    },
    at(40),
  )
  .expect("event");
  assert_eq!(
    db.chats().expect("chats")[0].messages,
    2,
    "History counts what was said, not actions and events"
  );
  let kinds: Vec<EntryKind> = db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .map(|entry| entry.kind)
    .collect();
  assert_eq!(
    kinds,
    [
      EntryKind::Person {
        message,
        text: body("What runs?"),
        turn: TurnState::Answered
      },
      EntryKind::Reply {
        text: "Two runs.".to_owned(),
        model: model()
      },
      EntryKind::Action(action),
      EntryKind::Event(ChatEvent::NewSession {
        because: SessionChange::Account
      }),
    ]
  );
  db.forget_chat_message(message).expect("nothing to forget");
  assert!(
    db.chat(chat).expect("read").is_some(),
    "an answered message is never forgotten"
  );
}

#[test]
fn history_lists_the_latest_chat_first_and_stale_turns_end_as_interrupted() {
  let (_repo, mut db, _run, _execution) = started_run();
  let (older, newer) = (ChatId::generate(), ChatId::generate());
  let (first, second) = (ChatMessageId::generate(), ChatMessageId::generate());
  db.keep_chat_message(
    &Chat::start(older, &body("Older"), at(10)),
    (first, &body("Older")),
    at(10),
  )
  .expect("kept");
  db.keep_chat_message(
    &Chat::start(newer, &body("Newer"), at(20)),
    (second, &body("Newer")),
    at(20),
  )
  .expect("kept");
  db.add_chat_reply(older, ("Later reply", &model()), at(30))
    .expect("reply");
  let listed: Vec<(ChatId, u32)> = db
    .chats()
    .expect("chats")
    .into_iter()
    .map(|listing| (listing.chat.id(), listing.messages))
    .collect();
  assert_eq!(listed, [(older, 2), (newer, 1)]);

  // A runner answers the newer conversation's turn: only the older one's is cut short.
  assert_eq!(
    db.interrupt_chat_turns(&[second], None).expect("interrupted"),
    [older]
  );
  assert_eq!(
    db.chat_turn(first).expect("read").expect("kept").turn,
    TurnState::Failed(TurnFailure::Interrupted)
  );
  assert!(db.chat_turn(second).expect("read").expect("kept").turn.is_thinking());
  assert_eq!(
    db.interrupt_chat_turns(&[], Some(older)).expect("nothing left"),
    [],
    "a stop in one conversation leaves the others"
  );
  assert_eq!(db.interrupt_chat_turns(&[], Some(newer)).expect("stopped"), [newer]);
}
