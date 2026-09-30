mod support;

use orchester_core::{
  id::IdempotencyKey,
  revision::Revision,
  run::{BranchName, NewRun, PendingToolApproval, ToolApprovalDecision},
  store::{ActivityStore, RunStore},
};
use support::{at, budget, model, started_run};

#[test]
fn project_pending_requests_span_runs_in_request_time_order() {
  let (_repo, mut db, first, first_execution) = started_run();
  let first_invocation = db
    .start_invocation(first.id(), first_execution, &model(), at(10))
    .expect("first invocation");
  let new = NewRun {
    task: first.task(),
    snapshot: first.snapshot().clone(),
    branch: BranchName::new("orchester/orc-1-run-2").expect("branch"),
    base: Revision::new(&"b".repeat(40)).expect("revision"),
    base_branch: None,
    budget: budget(1_000_000),
    max_duration: first.max_duration(),
  };
  let mut second = db
    .create_run(IdempotencyKey::generate(), new, at(20))
    .expect("second run");
  let events = second.start(at(21)).expect("start second");
  db.append_events(&second, &events).expect("append second");
  let second_execution = second.current_attempt().expect("attempt").executions()[0].id();
  let second_invocation = db
    .start_invocation(second.id(), second_execution, &model(), at(22))
    .expect("second invocation");
  let first_request = PendingToolApproval {
    id: format!("{}:1", first_invocation.id),
    run: first.id(),
    execution: first_execution,
    invocation: first_invocation.id,
    agent: "Designer".to_owned(),
    command: false,
    scope: None,
    server: "pencil".to_owned(),
    action: "batch_design".to_owned(),
    target: None,
    requested_at: at(25),
  };
  let second_request = PendingToolApproval {
    id: format!("{}:2", second_invocation.id),
    run: second.id(),
    execution: second_execution,
    invocation: second_invocation.id,
    agent: "Designer".to_owned(),
    command: false,
    scope: None,
    server: "pencil".to_owned(),
    action: "batch_design".to_owned(),
    target: None,
    requested_at: at(24),
  };
  db.request_tool_approval(&first_request).expect("first request");
  db.request_tool_approval(&second_request).expect("second request");
  assert_eq!(
    db.pending_tool_approvals_all().expect("all pending"),
    [second_request.clone(), first_request.clone()]
  );
  assert_eq!(
    db.pending_tool_approvals(first.id()).expect("first only"),
    std::slice::from_ref(&first_request)
  );
  assert!(
    db.interrupt_tool_approvals(second.id(), second_invocation.id)
      .expect("interrupt")
  );
  assert_eq!(db.pending_tool_approvals_all().expect("remaining"), [first_request]);
}

#[test]
fn one_tool_request_is_decided_once_and_no_longer_pending() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let approval = PendingToolApproval {
    id: format!("{}:42", invocation.id),
    run: run.id(),
    execution,
    invocation: invocation.id,
    agent: "Designer".to_owned(),
    command: true,
    scope: Some("One command outside the sandbox".to_owned()),
    server: "pencil".to_owned(),
    action: "batch_design".to_owned(),
    target: Some("/scratch/disposable.pen".to_owned()),
    requested_at: at(11),
  };
  db.request_tool_approval(&approval).expect("request");
  assert_eq!(
    db.pending_tool_approvals(run.id()).expect("pending"),
    std::slice::from_ref(&approval)
  );
  assert!(
    db.claim_tool_approval(run.id(), &approval.id, true, at(12))
      .expect("claim")
  );
  assert!(
    !db
      .claim_tool_approval(run.id(), &approval.id, false, at(13))
      .expect("repeat")
  );
  assert!(db.pending_tool_approvals(run.id()).expect("pending").is_empty());
  assert!(
    db.delivered_tool_approval(run.id(), &approval.id, ToolApprovalDecision::Approved)
      .expect("delivered")
  );
  assert!(
    !db
      .delivered_tool_approval(run.id(), &approval.id, ToolApprovalDecision::Denied)
      .expect("repeat")
  );
}

#[test]
fn interrupted_request_cannot_be_approved_after_restart() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let approval = PendingToolApproval {
    id: format!("{}:57", invocation.id),
    run: run.id(),
    execution,
    invocation: invocation.id,
    agent: "Designer".to_owned(),
    command: false,
    scope: None,
    server: "pencil".to_owned(),
    action: "batch_design".to_owned(),
    target: None,
    requested_at: at(11),
  };
  db.request_tool_approval(&approval).expect("request");
  assert!(db.interrupt_tool_approvals(run.id(), invocation.id).expect("interrupt"));
  assert!(db.has_interrupted_tool_approval(run.id()).expect("interrupted"));
  assert!(
    !db
      .claim_tool_approval(run.id(), &approval.id, true, at(12))
      .expect("stale")
  );
  assert!(db.pending_tool_approvals(run.id()).expect("pending").is_empty());
}

#[test]
fn claimed_approval_or_denial_is_interrupted_if_the_worker_dies_mid_delivery() {
  for approve in [true, false] {
    let (_repo, mut db, run, execution) = started_run();
    let invocation = db
      .start_invocation(run.id(), execution, &model(), at(10))
      .expect("invocation");
    let approval = PendingToolApproval {
      id: format!("{}:77", invocation.id),
      run: run.id(),
      execution,
      invocation: invocation.id,
      agent: "Designer".to_owned(),
      command: false,
      scope: None,
      server: "pencil".to_owned(),
      action: "batch_design".to_owned(),
      target: None,
      requested_at: at(11),
    };
    db.request_tool_approval(&approval).expect("request");
    assert!(
      db.claim_tool_approval(run.id(), &approval.id, approve, at(12))
        .expect("claim")
    );
    assert!(db.pending_tool_approvals(run.id()).expect("pending").is_empty());
    assert!(db.interrupt_tool_approvals(run.id(), invocation.id).expect("interrupt"));
    assert!(db.has_interrupted_tool_approval(run.id()).expect("interrupted"));
    assert!(
      !db
        .claim_tool_approval(run.id(), &approval.id, approve, at(13))
        .expect("stale")
    );
  }
}
