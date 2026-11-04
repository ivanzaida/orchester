//! The records of a task's conversation, agents' questions and their tool requests (`MessageRecord`,
//! `QuestionRecord`, `ApprovalRecord`), the values the runner's executor sends to the coordinator's sink (ORC-679):
//! each kind survives serialization unchanged, and written as it arrived it stores what the direct write stores, with
//! the store's own rules still in force: a delivery only moves forward, a racing change applies once, a request is
//! asked, answered and claimed once. `cargo test -p orchester-store --test decision_records decision_record`.

mod support;

use orchester_core::{
  conversation::{AgentMessage, Arrival, Message, MessageBody, Recipient, Undelivered},
  id::{IdempotencyKey, MessageId},
  question::{GivenAnswer, Question, QuestionOption, QuestionRequest, QuestionState, QuestionTurn},
  run::{
    AnswerKind, Answerer, CameToPerson, DecidedBy, DenialNote, Invocation, PendingToolApproval, PersonReason,
    RequestAnswer, RuleScope, ToolApprovalDecision, review::Review,
  },
  store::{ActivityStore, ConversationStore},
};
use orchester_store::{ApprovalRecord, MessageRecord, ProjectDb, QuestionRecord, StoreError};
use serde::{Serialize, de::DeserializeOwned};

use self::support::{at, model, started_run};

/// `record` sent as JSON and read back, as a sink in another process would receive it.
fn sent<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(record: &T) -> T {
  let text = serde_json::to_string(record).expect("a record serializes");
  let received: T = serde_json::from_str(&text).expect("a record deserializes");
  assert_eq!(&received, record, "the record survives being sent");
  received
}

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("a message")
}

/// A started run with its first invocation.
fn invoked() -> (tempfile::TempDir, ProjectDb, orchester_core::run::Run, Invocation) {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(9))
    .expect("invocation");
  (repo, db, run, invocation)
}

#[test]
fn decision_record_of_messages_stores_what_direct_writes_store() {
  let (_repo, mut db, run, invocation) = invoked();
  let mut message = Message::from_person(run.task(), Some(run.id()), Recipient::Orchestrator, body("Hi."), at(10));
  db.send_message(IdempotencyKey::generate(), &message).expect("stored");
  message
    .deliver(run.id(), (invocation.execution, invocation.id), at(11), Arrival::Prompt)
    .expect("delivered");
  assert!(
    db.write_message(&sent(&MessageRecord::deliveries(std::slice::from_ref(&message)).expect("a record")))
      .expect("written")
  );
  let delivered = message.clone();
  message.acknowledge(at(12)).expect("acknowledged");
  db.write_message(&sent(&MessageRecord::deliveries(std::slice::from_ref(&message)).expect("a record")))
    .expect("written");
  let reply = Message::from_agent(
    run.task(),
    run.id(),
    (invocation.execution, invocation.id),
    AgentMessage::Reply,
    body("Done."),
    at(13),
  );
  assert!(db.write_message(&sent(&MessageRecord::added(&reply))).expect("written"));
  assert_eq!(db.messages(run.task()).expect("messages"), [message.clone(), reply.clone()]);

  let backwards = MessageRecord::deliveries(std::slice::from_ref(&delivered)).expect("a record");
  assert!(
    db.write_message(&sent(&backwards)).is_err(),
    "a delivery only moves forward"
  );
  assert!(matches!(
    MessageRecord::deliveries(std::slice::from_ref(&reply)),
    Err(StoreError::Unrepresentable(_))
  ));
  assert_eq!(db.messages(run.task()).expect("messages"), [message, reply]);
}

#[test]
fn decision_record_of_a_racing_delivery_applies_once() {
  let (_repo, mut db, run, invocation) = invoked();
  let mut refused = Message::from_person(run.task(), Some(run.id()), Recipient::Orchestrator, body("Now."), at(10));
  db.send_message(IdempotencyKey::generate(), &refused).expect("stored");
  refused
    .not_delivered(run.id(), invocation.execution, at(11), Undelivered::Refused)
    .expect("not delivered");
  db.save_deliveries(std::slice::from_ref(&refused)).expect("saved");
  let (mut again, mut taken) = (refused.clone(), refused.clone());
  again.send_after_turn().expect("sent after the turn");
  taken
    .deliver(run.id(), (invocation.execution, invocation.id), at(12), Arrival::MidTurn)
    .expect("taken late");
  let first = sent(&MessageRecord::delivery_change(&refused, &again).expect("a record"));
  let later = sent(&MessageRecord::delivery_change(&refused, &taken).expect("a record"));
  assert!(db.write_message(&first).expect("written"), "the first change wins");
  assert!(
    !db.write_message(&later).expect("no store error"),
    "the later one finds it moved on and leaves it"
  );
  assert_eq!(db.messages(run.task()).expect("messages"), [again]);
}

fn request(invocation: &Invocation, key: &str, turn: QuestionTurn) -> QuestionRequest {
  let question = Question::new(
    "Where should the sign-in button go?",
    Some("Docs layout"),
    vec![QuestionOption::new("Top", None), QuestionOption::new("Bottom", None)],
    false,
  )
  .expect("a question");
  QuestionRequest::new(
    format!("{}:{key}", invocation.id),
    (invocation.run, invocation.execution, invocation.id),
    "Builder".to_owned(),
    vec![question],
    (at(20), turn),
  )
  .expect("a request")
}

#[test]
fn decision_record_of_questions_asks_answers_and_ends_each_once() {
  let (_repo, mut db, run, invocation) = invoked();
  let answered = request(&invocation, "1", QuestionTurn::StepEnd);
  let asked = sent(&QuestionRecord::asked(&answered));
  assert!(db.write_question(&asked).expect("asked"));
  assert!(db.write_question(&asked).is_err(), "an id is asked once in a run");
  assert_eq!(db.questions(run.id()).expect("questions")[0].request, answered);

  let given = answered
    .answer(&[GivenAnswer {
      choices: vec!["Top".to_owned()],
      text: None,
    }])
    .expect("answers");
  let message = MessageId::generate();
  let answer = sent(&QuestionRecord::answered(
    (run.id(), answered.id()),
    &given,
    (Answerer::Person, at(30)),
    Some(message),
  ));
  assert!(db.write_question(&answer).expect("answered"));
  assert!(!db.write_question(&answer).expect("not again"), "a request is answered once");
  let stored = db.question_request(run.id(), answered.id()).expect("read").expect("kept");
  assert_eq!(
    stored.state,
    QuestionState::Answered {
      answers: given,
      by: Answerer::Person,
      at: at(30),
    }
  );
  assert_eq!(stored.message, Some(message));
  assert!(
    !db.write_question(&sent(&QuestionRecord::ended(run.id(), answered.id())))
      .expect("written"),
    "an answered request no longer waits"
  );
  db.write_question(&sent(&QuestionRecord::undelivered(run.id(), answered.id())))
    .expect("written");

  let in_turn = request(&invocation, "2", QuestionTurn::InTurn);
  db.write_question(&sent(&QuestionRecord::asked(&in_turn))).expect("asked");
  let interrupted = sent(&QuestionRecord::interrupted(run.id(), invocation.id));
  assert!(db.write_question(&interrupted).expect("interrupted"));
  assert!(!db.write_question(&interrupted).expect("written"), "nothing waits any more");
  let states: Vec<_> = db
    .questions(run.id())
    .expect("questions")
    .into_iter()
    .map(|asked| matches!(asked.state, QuestionState::Interrupted { .. }))
    .collect();
  assert_eq!(states, [true, true]);
}

fn approval(invocation: &Invocation, key: &str) -> PendingToolApproval {
  PendingToolApproval {
    id: format!("{}:{key}", invocation.id),
    run: invocation.run,
    execution: invocation.execution,
    invocation: invocation.id,
    agent: "Designer".to_owned(),
    command: false,
    scope: Some("{\"nodes\":1}".to_owned()),
    server: "pencil".to_owned(),
    action: "batch_design".to_owned(),
    target: None,
    requested_at: at(20),
    native: None,
  }
}

#[test]
fn decision_record_of_tool_requests_is_written_as_directly() {
  let (_repo, mut db, run, invocation) = invoked();
  let rule = db
    .add_approval_rule(
      run.id(),
      &RuleScope::SimilarMcpTool {
        server: "pencil".to_owned(),
        tool: "batch_design".to_owned(),
        folder: None,
        check: None,
      },
      at(10),
    )
    .expect("rule");
  let (approved, denied, ended) = (
    approval(&invocation, "1"),
    approval(&invocation, "2"),
    approval(&invocation, "3"),
  );
  for request in [&approved, &denied, &ended] {
    assert!(db.write_approval(&sent(&ApprovalRecord::requested(request))).expect("requested"));
  }
  assert!(
    db.write_approval(&sent(&ApprovalRecord::requested(&approved))).is_err(),
    "a request is recorded once"
  );
  assert_eq!(db.pending_tool_approvals(run.id()).expect("pending").len(), 3);

  let (id, run_id) = (approved.id.as_str(), run.id());
  let came = CameToPerson::because(PersonReason::Outside);
  let review = Review::Approved {
    reason: "It serves the task.".to_owned(),
    model: "claude-haiku-4-5-20251001".to_owned(),
  };
  for record in [
    ApprovalRecord::came_to_person(run_id, id, Some(&came)),
    ApprovalRecord::reviewing(run_id, id, true),
  ] {
    db.write_approval(&sent(&record)).expect("written");
  }
  assert_eq!(db.came_to_person(run_id, id).expect("read"), Some(came));
  assert_eq!(db.reviewing_tool_approvals().expect("read"), [approved.id.clone()]);
  db.write_approval(&sent(&ApprovalRecord::reviewed(run_id, id, &review)))
    .expect("written");
  assert!(db.reviewing_tool_approvals().expect("read").is_empty());
  assert_eq!(db.tool_approval_review(run_id, id).expect("read"), Some(review.clone()));

  let claimed = sent(&ApprovalRecord::claimed(run_id, id, DecidedBy::SimilarRule, at(30)));
  assert!(db.write_approval(&claimed).expect("claimed"));
  assert!(!db.write_approval(&claimed).expect("written"), "a request is claimed once");
  let answer = RequestAnswer::by(AnswerKind::Person);
  for record in [
    ApprovalRecord::answer(run_id, id, &answer),
    ApprovalRecord::rule(run_id, id, rule.id),
  ] {
    db.write_approval(&sent(&record)).expect("written");
  }
  let delivered = sent(&ApprovalRecord::delivered(run_id, id, ToolApprovalDecision::Approved));
  assert!(db.write_approval(&delivered).expect("delivered"));
  assert!(!db.write_approval(&delivered).expect("written"), "delivered once");

  let note = DenialNote::new("Use serde_json instead.").expect("words");
  for record in [
    ApprovalRecord::claimed(run_id, &denied.id, DecidedBy::PersonDeny, at(31)),
    ApprovalRecord::denial_note(run_id, &denied.id, &note),
  ] {
    db.write_approval(&sent(&record)).expect("written");
  }
  assert!(
    db.write_approval(&sent(&ApprovalRecord::ended(run_id, &ended.id)))
      .expect("ended")
  );

  // The denial claimed and not delivered ends interrupted; the delivered approval leaves the run to inspect.
  assert!(
    db.write_approval(&sent(&ApprovalRecord::interrupted(run_id, invocation.id)))
      .expect("interrupted")
  );
  assert!(db.has_interrupted_tool_approval(run_id).expect("read"));
  assert_eq!(db.approval_rules(run_id).expect("rules")[0].answered, 1);
  assert_eq!(db.request_answers(run_id).expect("answers")[0].answer, Some(answer));
  let decided = db.decided_tool_approvals(run_id).expect("decided");
  let summary: Vec<_> = decided
    .iter()
    .map(|decided| (decided.request.id.as_str(), decided.decision, decided.decided_by))
    .collect();
  assert_eq!(
    summary,
    [
      (id, Some(ToolApprovalDecision::Approved), Some(DecidedBy::SimilarRule)),
      (denied.id.as_str(), None, Some(DecidedBy::PersonDeny)),
      (ended.id.as_str(), None, None),
    ]
  );
  assert_eq!(decided[0].review, Some(review));
  assert_eq!(decided[1].note, Some(note));
}
