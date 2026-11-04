//! A conversation message as it travels to a run's executor (the coordinator link, ORC-667): it comes back the same
//! from its JSON, the person's side's markings included, and JSON that breaks a rule a stored message keeps is refused.

use orchester_core::{
  conversation::{AgentMessage, Arrival, Message, MessageBody, Recipient, Timing},
  id::{ChatId, InvocationId, ResourceId, RunId, StepExecutionId, TaskId},
  run::EventSequence,
};
use serde_json::Value;

use crate::fixture::at;

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("a message")
}

fn round_trip(message: &Message) {
  let json = serde_json::to_string(message).expect("the message serializes");
  let back: Message = serde_json::from_str(&json).expect("the JSON reads back");
  assert_eq!(&back, message, "{json}");
}

/// `message` as JSON with `field` set to `value`, read back.
fn with(message: &Message, field: &str, value: Value) -> Result<Message, serde_json::Error> {
  let mut json = serde_json::to_value(message).expect("the message serializes");
  json[field] = value;
  serde_json::from_value(json)
}

#[test]
fn conversation_wire_messages_come_back_the_same_with_their_markings() {
  let (task, run) = (TaskId::generate(), RunId::generate());
  let queued = Message::from_person(task, None, Recipient::Orchestrator, body("Keep email sign-in."), at(40));
  round_trip(&queued);
  let marked = Message::from_person(
    task,
    Some(run),
    Recipient::Agent(ResourceId::generate()),
    body("Try again."),
    at(41),
  )
  .sent(Timing::Now)
  .and_then(|message| message.recovering_after(EventSequence::FIRST))
  .and_then(|message| message.sent_by_orchestrator(Some(ChatId::generate())))
  .expect("a person's message takes every marking");
  round_trip(&marked);
  let mut delivered = marked;
  let to = (StepExecutionId::generate(), InvocationId::generate());
  delivered.deliver(run, to, at(42), Arrival::MidTurn).expect("delivered");
  round_trip(&delivered);
  let reply = Message::from_agent(task, run, to, AgentMessage::Reply, body("Done."), at(43));
  round_trip(&reply);
}

#[test]
fn conversation_wire_refuses_what_a_stored_message_never_holds() {
  let (task, run) = (TaskId::generate(), RunId::generate());
  let to = (StepExecutionId::generate(), InvocationId::generate());
  let reply = Message::from_agent(task, run, to, AgentMessage::Question, body("Which one?"), at(43));
  assert!(
    with(&reply, "run", Value::Null).is_err(),
    "an agent's message names its run"
  );
  assert!(
    with(&reply, "by_automation", Value::Bool(true)).is_err(),
    "an agent's message is never automation's"
  );
  assert!(
    with(&reply, "timing", serde_json::json!("Now")).is_err(),
    "an agent's message is never sent now"
  );
  let queued = Message::from_person(task, None, Recipient::Orchestrator, body("Keep it."), at(40));
  assert!(
    with(&queued, "recovery_after", serde_json::json!(1)).is_err(),
    "only a message of a run continues it"
  );
  assert!(
    with(&queued, "body", serde_json::json!("  ")).is_err(),
    "a message says something"
  );
}
