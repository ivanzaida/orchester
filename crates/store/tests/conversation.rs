//! Task conversations: a person's messages stored once per key, their delivery moving forward only,
//! agents' replies and questions, the runs queued messages wait for, and follow-up attempts of a
//! succeeded run.

mod support;

use orchester_core::{
  conversation::{AgentMessage, Delivery, Message, MessageBody, Recipient, Sender},
  id::{IdempotencyKey, InvocationId, StepExecutionId},
  run::{AttemptReason, Run, RunStatus},
  store::{ActivityStore, ConversationStore, RunStore},
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection;

use self::support::{at, model, started_run};

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("a message")
}

fn raw(db: ProjectDb, repo: &std::path::Path) -> Connection {
  drop(db);
  Connection::open(repo.join(".orchester").join("orchester.db")).expect("open")
}

/// A started run whose step is working in its first invocation.
fn working() -> (tempfile::TempDir, ProjectDb, Run, StepExecutionId, InvocationId) {
  let (repo, mut db, mut run, execution) = started_run();
  let events = run.start_step(execution, at(8)).expect("start");
  db.append_events(&run, &events).expect("append");
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(9))
    .expect("invocation")
    .id;
  (repo, db, run, execution, invocation)
}

#[test]
fn conversation_a_person_s_message_is_stored_once_per_key() {
  let (_repo, mut db, run, ..) = working();
  let key = IdempotencyKey::generate();
  let first = Message::from_person(run.task(), Some(run.id()), Recipient::Orchestrator, body("Hi."), at(10));
  assert_eq!(db.send_message(key, &first).expect("stored"), first);
  let retry = Message::from_person(run.task(), Some(run.id()), Recipient::Orchestrator, body("Hi."), at(11));
  assert_eq!(db.send_message(key, &retry).expect("the first"), first);
  assert_eq!(db.messages(run.task()).expect("messages"), [first]);
}

#[test]
fn conversation_a_message_needs_an_existing_task() {
  let (_repo, mut db, ..) = working();
  let orphan = Message::from_person(
    orchester_core::id::TaskId::generate(),
    None,
    Recipient::Orchestrator,
    body("Hi."),
    at(10),
  );
  assert!(matches!(
    db.send_message(IdempotencyKey::generate(), &orphan),
    Err(StoreError::TaskNotFound(_))
  ));
}

#[test]
fn conversation_delivery_is_stored_and_moves_forward_only() {
  let (repo, mut db, run, execution, invocation) = working();
  // Written before the task had a run; delivered, it belongs to the run it went to.
  let mut message = Message::from_person(run.task(), None, Recipient::Orchestrator, body("Keep email."), at(10));
  db.send_message(IdempotencyKey::generate(), &message).expect("stored");
  message
    .deliver(run.id(), (execution, invocation), at(12))
    .expect("delivered");
  db.save_deliveries(std::slice::from_ref(&message)).expect("saved");
  assert_eq!(db.messages(run.task()).expect("messages"), [message.clone()]);
  message.acknowledge(at(13)).expect("acknowledged");
  db.save_deliveries(std::slice::from_ref(&message)).expect("saved");
  let stored = db.messages(run.task()).expect("messages");
  assert_eq!(stored, [message.clone()]);
  assert!(matches!(
    stored[0].delivery(),
    Some(Delivery::Acknowledged { at: acknowledged, .. }) if acknowledged == at(13)
  ));
  let connection = raw(db, repo.path());
  let id = message.id().to_string();
  for statement in [
    "UPDATE messages SET acknowledged_at = NULL WHERE id = ?1",
    "UPDATE messages SET delivered_at = 99 WHERE id = ?1",
    "UPDATE messages SET body = 'Something else.' WHERE id = ?1",
    "DELETE FROM messages WHERE id = ?1",
  ] {
    assert!(
      connection.execute(statement, [&id]).is_err(),
      "{statement} must be refused"
    );
  }
}

#[test]
fn conversation_runs_with_queued_messages_are_the_runs_they_were_written_for() {
  let (_repo, mut db, run, execution, invocation) = working();
  let early = Message::from_person(run.task(), None, Recipient::Orchestrator, body("Early."), at(10));
  db.send_message(IdempotencyKey::generate(), &early).expect("stored");
  assert!(db.runs_with_queued_messages().expect("runs").is_empty(), "no run named");
  let mut queued = Vec::new();
  for (text, millis) in [("Later.", 11), ("Again.", 12)] {
    let message = Message::from_person(
      run.task(),
      Some(run.id()),
      Recipient::Orchestrator,
      body(text),
      at(millis),
    );
    queued.push(db.send_message(IdempotencyKey::generate(), &message).expect("stored"));
  }
  assert_eq!(db.runs_with_queued_messages().expect("runs"), [run.id()], "listed once");
  for message in &mut queued {
    message
      .deliver(run.id(), (execution, invocation), at(13))
      .expect("delivered");
  }
  db.save_deliveries(&queued).expect("saved");
  assert!(db.runs_with_queued_messages().expect("runs").is_empty());
}

#[test]
fn conversation_agents_messages_follow_the_person_s_in_order() {
  let (_repo, mut db, run, execution, invocation) = working();
  let reviewer = orchester_core::id::ResourceId::generate();
  let question = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Agent(reviewer),
    body("Why?"),
    at(10),
  );
  db.send_message(IdempotencyKey::generate(), &question).expect("stored");
  let reply = Message::from_agent(
    run.task(),
    run.id(),
    (execution, invocation),
    AgentMessage::Reply,
    body("Because."),
    at(11),
  );
  db.add_agent_message(&reply).expect("stored");
  let asked = Message::from_agent(
    run.task(),
    run.id(),
    (execution, invocation),
    AgentMessage::Question,
    body("Keep email sign-in?"),
    at(12),
  );
  db.add_agent_message(&asked).expect("stored");
  let stored = db.messages(run.task()).expect("messages");
  assert_eq!(stored, [question, reply, asked]);
  assert!(matches!(
    stored[0].sender(),
    Sender::Person { recipient: Recipient::Agent(profile), .. } if *profile == reviewer
  ));
  assert!(matches!(
    stored[2].sender(),
    Sender::Agent {
      kind: AgentMessage::Question,
      ..
    }
  ));
}

#[test]
fn conversation_a_follow_up_attempt_round_trips() {
  let (_repo, mut db, mut run, execution, _) = working();
  let events = run.complete_step(execution, at(20)).expect("passed");
  db.append_events(&run, &events).expect("append");
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let step = run.follow_up_step(None).expect("an agent step");
  let events = run.follow_up(&step, at(30)).expect("followed up");
  db.append_events(&run, &events).expect("append");
  let stored = db.run(run.id()).expect("read").expect("run");
  assert_eq!(stored, run);
  assert_eq!(
    stored.current_attempt().map(|attempt| attempt.reason().clone()),
    Some(AttemptReason::FollowUp { step })
  );
  let follow_up = stored.current_attempt().expect("attempt").executions()[0].id();
  assert_eq!(stored.followed_up(follow_up), Some(execution));
  assert_eq!(Run::replay(&db.events(run.id()).expect("events")), Ok(run));
}
