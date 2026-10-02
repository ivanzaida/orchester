//! Task conversations: a person's messages stored once per key, their delivery moving forward only,
//! agents' replies and questions, the runs queued messages wait for, and follow-up attempts of a
//! succeeded run.

mod support;

use orchester_core::{
  conversation::{AgentMessage, Arrival, Delivery, Message, MessageBody, Recipient, Sender, Timing, Undelivered},
  id::{IdempotencyKey, InvocationId, StepExecutionId},
  run::{AttemptReason, Run, RunStatus},
  store::{ActivityStore, ConversationStore, RunStore},
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection;

use self::support::{at, copied_at_version, model, started_run};

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
    .deliver(run.id(), (execution, invocation), at(12), Arrival::Prompt)
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
      .deliver(run.id(), (execution, invocation), at(13), Arrival::Prompt)
      .expect("delivered");
  }
  db.save_deliveries(&queued).expect("saved");
  assert!(db.runs_with_queued_messages().expect("runs").is_empty());
}

#[test]
fn conversation_a_message_sent_now_keeps_how_it_arrived_or_why_it_did_not() {
  let (repo, mut db, run, execution, invocation) = working();
  let send_now = |text: &str, millis| {
    Message::from_person(
      run.task(),
      Some(run.id()),
      Recipient::Orchestrator,
      body(text),
      at(millis),
    )
    .sent(Timing::Now)
    .expect("a person's message")
  };
  let mut steered = send_now("Stop editing the parser.", 10);
  let mut refused = send_now("Use the new API.", 11);
  for message in [&steered, &refused] {
    db.send_message(IdempotencyKey::generate(), message).expect("stored");
  }
  assert_eq!(
    db.messages(run.task()).expect("messages"),
    [steered.clone(), refused.clone()]
  );
  steered
    .deliver(run.id(), (execution, invocation), at(12), Arrival::MidTurn)
    .expect("delivered");
  refused
    .not_delivered(run.id(), execution, at(13), Undelivered::Refused)
    .expect("not delivered");
  db.save_deliveries(&[steered.clone(), refused.clone()]).expect("saved");
  assert_eq!(
    db.messages(run.task()).expect("messages"),
    [steered.clone(), refused.clone()]
  );
  assert!(
    db.runs_with_queued_messages().expect("runs").is_empty(),
    "a message that was not delivered is not queued"
  );
  // The person sends it after the turn: queued again.
  refused.send_after_turn().expect("sent after the turn");
  db.save_deliveries(std::slice::from_ref(&refused)).expect("saved");
  let stored = db.messages(run.task()).expect("messages");
  assert_eq!(stored[1], refused);
  assert_eq!(
    (stored[1].timing(), stored[1].delivery()),
    (Timing::AfterTurn, Some(Delivery::Queued))
  );
  assert_eq!(db.runs_with_queued_messages().expect("runs"), [run.id()]);
  let connection = raw(db, repo.path());
  assert!(
    connection
      .execute(
        "UPDATE messages SET timing = 'now' WHERE id = ?1",
        [refused.id().to_string()]
      )
      .is_err(),
    "a message sent after the turn is not sent now again"
  );
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

#[test]
fn conversation_recovery_intent_round_trips_and_cannot_be_rewritten() {
  let (repo, mut db, mut run, execution, invocation) = working();
  let events = run
    .fail_step(execution, orchester_core::run::StepFailure::ReportedFailed, at(20))
    .expect("failed");
  db.append_events(&run, &events).expect("stored");
  let mut message = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    body("Continue."),
    at(20),
  )
  .recovering_after(run.last_sequence())
  .expect("recovery");
  db.send_message(IdempotencyKey::generate(), &message).expect("stored");
  let step = run.recovery_step(None).expect("agent");
  let events = run.recover(&step, at(21)).expect("recovered");
  db.append_events(&run, &events).expect("stored attempt");
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
  message
    .deliver(run.id(), (execution, invocation), at(22), Arrival::Prompt)
    .expect("delivered");
  db.save_deliveries(std::slice::from_ref(&message)).expect("delivery");
  message.acknowledge(at(23)).expect("acknowledge");
  db.save_deliveries(std::slice::from_ref(&message))
    .expect("acknowledged");
  assert_eq!(db.messages(run.task()).expect("read"), [message]);
  let connection = raw(db, repo.path());
  assert!(
    connection
      .execute("UPDATE messages SET recovery_sequence = NULL", [])
      .is_err()
  );
  assert!(
    connection
      .execute("UPDATE messages SET recovery_sequence = recovery_sequence + 1", [])
      .is_err()
  );
}

/// The migrations before the recovery migration; migrations are append-only, so this stays.
const BEFORE_RECOVERY: u32 = 27;

#[test]
fn conversation_recovery_migration_preserves_existing_failed_run_and_messages() {
  let (repo, mut db, mut run, execution, _) = working();
  let events = run
    .fail_step(execution, orchester_core::run::StepFailure::ReportedFailed, at(20))
    .expect("failed");
  db.append_events(&run, &events).expect("stored failure");
  let message = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    body("Earlier message."),
    at(19),
  );
  db.send_message(IdempotencyKey::generate(), &message).expect("stored");
  drop(db);
  // The same rows in a database as it was before the recovery migration.
  let current = repo.path().join(".orchester").join("orchester.db");
  let repo = copied_at_version(&current, BEFORE_RECOVERY);
  let mut db = ProjectDb::open(repo.path()).expect("migrate existing project");
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
  assert_eq!(db.messages(run.task()).expect("messages"), [message]);
  let step = run.recovery_step(None).expect("agent");
  let events = run.recover(&step, at(21)).expect("recovery");
  db.append_events(&run, &events).expect("new attempt after migration");
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
}

/// The migrations before the steering migration (ORC-61); migrations are append-only, so this stays.
const BEFORE_STEERING: u32 = 36;

#[test]
fn conversation_steering_migration_keeps_queued_and_delivered_messages() {
  let (repo, mut db, run, execution, invocation) = working();
  let message = |text: &str, millis| {
    Message::from_person(
      run.task(),
      Some(run.id()),
      Recipient::Orchestrator,
      body(text),
      at(millis),
    )
  };
  let (mut delivered, queued) = (message("Delivered.", 10), message("Queued.", 11));
  for message in [&delivered, &queued] {
    db.send_message(IdempotencyKey::generate(), message).expect("stored");
  }
  delivered
    .deliver(run.id(), (execution, invocation), at(12), Arrival::Prompt)
    .expect("delivered");
  db.save_deliveries(std::slice::from_ref(&delivered)).expect("saved");
  drop(db);
  // The same rows in a database as it was before the steering migration.
  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_STEERING);
  let mut db = ProjectDb::open(old.path()).expect("migrate existing project");
  // Sent after the turn, as every message was; one delivered then came with an invocation's prompt.
  assert_eq!(db.messages(run.task()).expect("messages"), [delivered, queued]);
  assert_eq!(db.runs_with_queued_messages().expect("runs"), [run.id()]);
  let now = message("Sent now.", 13).sent(Timing::Now).expect("a person's message");
  db.send_message(IdempotencyKey::generate(), &now)
    .expect("a message sent now after the migration");
}

#[test]
fn conversation_a_change_of_delivery_is_kept_only_while_the_message_stands_as_read() {
  let (_repo, mut db, run, execution, invocation) = working();
  let mut refused = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    body("Use the new API."),
    at(10),
  )
  .sent(Timing::Now)
  .expect("a person's message");
  db.send_message(IdempotencyKey::generate(), &refused).expect("stored");
  refused
    .not_delivered(run.id(), execution, at(11), Undelivered::Refused)
    .expect("not delivered");
  db.save_deliveries(std::slice::from_ref(&refused)).expect("saved");
  // The desktop and the runner both read it as not delivered.
  let (mut again, mut taken) = (refused.clone(), refused.clone());
  again.send_after_turn().expect("sent after the turn");
  taken
    .deliver(run.id(), (execution, invocation), at(12), Arrival::MidTurn)
    .expect("taken late");
  assert!(
    db.change_delivery(&refused, &again).expect("stored"),
    "the first change wins"
  );
  assert!(
    !db.change_delivery(&refused, &taken).expect("no store error"),
    "the later one finds it moved on and leaves it"
  );
  assert_eq!(db.messages(run.task()).expect("messages"), [again]);
}
