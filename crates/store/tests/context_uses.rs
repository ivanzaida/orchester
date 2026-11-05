//! How full each agent session's context is (ORC-304): an invocation's latest reading in place of the one before, read
//! back by run; the Orchestrator chat's for the session its turns continue, forgotten when another session starts. The
//! window a CLI reported for a session is read back for the next one on the same model (ORC-704).
//! `cargo test -p orchester-store --test context_uses context_uses`.
mod support;

use orchester_core::{
  connection::context_use::ContextUse,
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId},
  orchestrator::Chat,
  resource::ModelId,
  store::ActivityStore,
};
use support::{at, model, started_run};

#[test]
fn context_uses_of_an_invocation_keep_the_latest_reading() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let silent = db
    .start_invocation(run.id(), execution, &model(), at(20))
    .expect("invocation");
  db.save_invocation_context(invocation.id, &ContextUse::of_model(6_000, "claude-haiku-4-5"), at(11))
    .expect("kept");
  db.save_invocation_context(invocation.id, &ContextUse::reported(9_000, 1_000_000), at(12))
    .expect("kept again");
  let read = db.invocation_contexts(run.id()).expect("read");
  assert_eq!(read.get(&invocation.id), Some(&ContextUse::reported(9_000, 1_000_000)));
  assert_eq!(read.get(&silent.id), None, "a CLI that reported nothing has no reading");
  db.save_invocation_context(silent.id, &ContextUse::unknown_window(40), at(21))
    .expect("kept");
  assert_eq!(
    db.invocation_contexts(run.id()).expect("read").get(&silent.id),
    Some(&ContextUse::unknown_window(40))
  );
}

#[test]
fn context_uses_of_a_chat_belong_to_its_session() {
  let (_repo, mut db, _run, _execution) = started_run();
  let chat = ChatId::generate();
  let text = MessageBody::new("What runs?").expect("text");
  db.keep_chat_message(
    &Chat::start(chat, &text, at(10)),
    (ChatMessageId::generate(), &text),
    at(10),
  )
  .expect("kept");
  let on = (ConnectionId::generate(), AgentAccountId::generate());
  assert_eq!(
    db.chat_context(chat).expect("read"),
    None,
    "unknown until a turn reports it"
  );
  db.set_chat_session(chat, "session-1", on).expect("session");
  let reading = ContextUse::of_model(30_000, "claude-opus-4-6");
  db.set_chat_context(chat, &reading).expect("kept");
  db.set_chat_session(chat, "session-1", on).expect("same session");
  assert_eq!(
    db.chat_context(chat).expect("read"),
    Some(reading),
    "the same session keeps it"
  );
  db.set_chat_session(chat, "session-2", on).expect("another session");
  assert_eq!(
    db.chat_context(chat).expect("read"),
    None,
    "another session starts empty"
  );
}

/// A kept conversation whose turns continue `session-1`, with what they run on.
fn chat_on_a_session(db: &mut orchester_store::ProjectDb) -> (ChatId, (ConnectionId, AgentAccountId)) {
  let chat = ChatId::generate();
  let text = MessageBody::new("What runs?").expect("text");
  db.keep_chat_message(
    &Chat::start(chat, &text, at(10)),
    (ChatMessageId::generate(), &text),
    at(10),
  )
  .expect("kept");
  let on = (ConnectionId::generate(), AgentAccountId::generate());
  db.set_chat_session(chat, "session-1", on).expect("session");
  (chat, on)
}

#[test]
fn context_uses_carry_a_chats_reported_window_on_its_model_only() {
  let (_repo, mut db, _run, _execution) = started_run();
  let (chat, on) = chat_on_a_session(&mut db);
  let opus = ModelId::new("claude-opus-5-5").expect("model");
  let sonnet = ModelId::new("claude-sonnet-5-5").expect("model");
  assert_eq!(
    db.chat_reported_window(chat, &opus).expect("read"),
    None,
    "none reported yet"
  );
  db.set_chat_reported_window(chat, &opus, 1_000_000).expect("kept");
  db.set_chat_session(chat, "session-1", on).expect("same session");
  assert_eq!(
    db.chat_reported_window(chat, &opus).expect("read"),
    Some(1_000_000),
    "the next turn on the same session and model carries it"
  );
  assert_eq!(
    db.chat_reported_window(chat, &sonnet).expect("read"),
    None,
    "a turn on another model does not carry the previous model's window"
  );
  db.set_chat_reported_window(chat, &sonnet, 200_000).expect("kept");
  assert_eq!(
    db.chat_reported_window(chat, &opus).expect("read"),
    None,
    "switched back: reported again"
  );
  db.set_chat_session(chat, "session-2", on).expect("another session");
  assert_eq!(
    db.chat_reported_window(chat, &sonnet).expect("read"),
    None,
    "another session forgets it"
  );
}

#[test]
fn context_uses_carry_a_resumed_sessions_reported_window_on_its_model_only() {
  let (_repo, mut db, run, execution) = started_run();
  let mut first = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  first.session = Some("session-1".to_owned());
  db.save_invocation(&first).expect("session");
  let other = ModelId::new("claude-other-model").expect("model");
  assert_eq!(
    db.session_reported_window(run.id(), "session-1", &model())
      .expect("read"),
    None,
    "nothing reported yet"
  );
  db.save_invocation_context(first.id, &ContextUse::of_model(9_000, model().as_str()), at(11))
    .expect("kept");
  assert_eq!(
    db.session_reported_window(run.id(), "session-1", &model())
      .expect("read"),
    None,
    "the table's estimate is not carried"
  );
  db.save_invocation_context(first.id, &ContextUse::reported(9_000, 1_000_000), at(12))
    .expect("kept");
  assert_eq!(
    db.session_reported_window(run.id(), "session-1", &model())
      .expect("read"),
    Some(1_000_000)
  );
  assert_eq!(
    db.session_reported_window(run.id(), "session-1", &other).expect("read"),
    None,
    "another model does not carry it"
  );
  assert_eq!(
    db.session_reported_window(run.id(), "session-2", &model())
      .expect("read"),
    None,
    "another session does not carry it"
  );
}
