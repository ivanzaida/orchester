//! How full each agent session's context is (ORC-304): an invocation's latest reading in place of the one before, read
//! back by run; the Orchestrator chat's for the session its turns continue, forgotten when another session starts.
//! `cargo test -p orchester-store --test context_uses context_uses`.
mod support;

use orchester_core::{
  connection::context_use::ContextUse,
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId},
  orchestrator::Chat,
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
