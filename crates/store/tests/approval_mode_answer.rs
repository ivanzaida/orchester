//! How each tool request was answered by its run's approval mode and why one came to the person, as the project
//! database keeps them (ORC-346, `docs/design/approval-modes.md`). Run with
//! `cargo test -p orchester-store --test approval_mode_answer approval_mode_answer`.

mod support;

use orchester_core::{
  run::{
    AnswerKind, AnswerRecord, CameToPerson, DecidedBy, PendingToolApproval, PersonReason, RequestAnswer,
    ToolApprovalDecision, review::ReviewFailure,
  },
  store::ActivityStore,
};

use self::support::{at, model, started_run};

#[test]
fn approval_mode_answer_keeps_how_and_why_each_request_was_answered() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let request = |number: u32, action: &str| PendingToolApproval {
    id: format!("{}:{number}", invocation.id),
    run: run.id(),
    execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: action.to_owned(),
    target: None,
    requested_at: at(20 + i64::from(number)),
    native: None,
  };
  let (inside, outside, reviewed) = (
    request(1, "cargo test"),
    request(2, "cat ../../../README.md"),
    request(3, "cargo build"),
  );
  for asked in [&inside, &outside, &reviewed] {
    db.request_tool_approval(asked).expect("recorded");
  }

  // Autopilot approves the first by its rule; the second comes to the person; the reviewer approves the third.
  let autopilot = RequestAnswer::by(AnswerKind::Autopilot);
  db.claim_tool_approval(run.id(), &inside.id, DecidedBy::AutoApproved, at(30))
    .expect("claimed");
  db.record_request_answer(run.id(), &inside.id, &autopilot)
    .expect("answer");
  let came = CameToPerson {
    detail: Some("/repo/README.md".to_owned()),
    ..CameToPerson::because(PersonReason::Outside)
  };
  db.record_came_to_person(run.id(), &outside.id, Some(&came))
    .expect("came to the person");
  assert_eq!(
    db.came_to_person(run.id(), &outside.id).expect("read"),
    Some(came.clone())
  );
  assert_eq!(db.came_to_person(run.id(), &inside.id).expect("read"), None);
  let reviewer = RequestAnswer::reviewer("Serves the task.");
  db.claim_tool_approval(run.id(), &reviewed.id, DecidedBy::Reviewer, at(31))
    .expect("claimed");
  db.record_request_answer(run.id(), &reviewed.id, &reviewer)
    .expect("answer");

  let record =
    |request: &PendingToolApproval, answer: Option<&RequestAnswer>, came: Option<&CameToPerson>| AnswerRecord {
      request: request.id.clone(),
      answer: answer.cloned(),
      came_to_person: came.cloned(),
    };
  assert_eq!(
    db.request_answers(run.id()).expect("answers"),
    [
      record(&inside, Some(&autopilot), None),
      record(&outside, None, Some(&came)),
      record(&reviewed, Some(&reviewer), None),
    ]
  );

  // The person answers the one that came to them; why it came stays with it.
  let person = RequestAnswer::by(AnswerKind::Person);
  db.claim_tool_approval(run.id(), &outside.id, DecidedBy::PersonDeny, at(40))
    .expect("claimed");
  db.record_request_answer(run.id(), &outside.id, &person)
    .expect("answer");
  db.delivered_tool_approval(run.id(), &outside.id, ToolApprovalDecision::Denied)
    .expect("delivered");
  assert_eq!(
    db.request_answers(run.id()).expect("answers")[1],
    record(&outside, Some(&person), Some(&came))
  );
  // A request no longer waiting keeps its reason: only a waiting one's is changed.
  db.record_came_to_person(run.id(), &outside.id, None)
    .expect("cleared only while waiting");
  assert_eq!(db.came_to_person(run.id(), &outside.id).expect("read"), Some(came));
}

#[test]
fn approval_mode_answer_reason_of_a_waiting_request_is_cleared_when_its_run_drops_to_manual() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let waiting = PendingToolApproval {
    id: format!("{}:1", invocation.id),
    run: run.id(),
    execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: "git push origin HEAD".to_owned(),
    target: None,
    requested_at: at(20),
    native: None,
  };
  db.request_tool_approval(&waiting).expect("recorded");
  let push = CameToPerson::because(PersonReason::Push);
  db.record_came_to_person(run.id(), &waiting.id, Some(&push))
    .expect("came to the person");
  db.record_came_to_person(run.id(), &waiting.id, None)
    .expect("looked at again");
  assert_eq!(db.came_to_person(run.id(), &waiting.id).expect("read"), None);
}

/// How the reviewer's review failed and the mode a request came by are kept with why it came (ORC-586), and given
/// back by the run's answers; a reason written before they were kept, which is what one without them is written as,
/// reads back without them.
#[test]
fn approval_mode_answer_keeps_how_the_review_failed() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let kinds = [
    CameToPerson::reviewer_failed(ReviewFailure::TimedOut).in_mode(true),
    CameToPerson::reviewer_failed(ReviewFailure::Unreadable),
    CameToPerson::reviewer_failed(ReviewFailure::CouldNotRun).in_mode(true),
    CameToPerson::because(PersonReason::ReviewerCouldNotRun),
  ];
  for (number, came) in kinds.iter().enumerate() {
    let waiting = PendingToolApproval {
      id: format!("{}:{number}", invocation.id),
      run: run.id(),
      execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: false,
      scope: None,
      server: "pencil".to_owned(),
      action: "batch_design".to_owned(),
      target: None,
      requested_at: at(20 + i64::try_from(number).expect("a short list")),
      native: None,
    };
    db.request_tool_approval(&waiting).expect("recorded");
    db.record_came_to_person(run.id(), &waiting.id, Some(came))
      .expect("came to the person");
    assert_eq!(
      db.came_to_person(run.id(), &waiting.id).expect("read").as_ref(),
      Some(came)
    );
  }
  let kept: Vec<Option<CameToPerson>> = db
    .request_answers(run.id())
    .expect("answers")
    .into_iter()
    .map(|record| record.came_to_person)
    .collect();
  assert_eq!(kept, kinds.map(Some));
}
