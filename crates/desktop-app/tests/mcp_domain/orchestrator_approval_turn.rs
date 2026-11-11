//! The Orchestrator takes up an agent's tool request while it drives (decided by the person on 2026-11-11), on the
//! seeded Kontur's cards: in the turn a request's wait started by itself, `decide_approval` decides only that request
//! (`not_this_request` for another), and the decision is kept as the chat's action naming the request's task; a turn
//! the person's message started may decide any. The tool keeps its own rules (`approve_once` or `deny`); the chat's
//! answer needs its reason, and the drive switch is its consent, not *Let automation answer approvals*.
//! `cargo test -p orchester-app --test mcp_domain orchestrator_approval_turn`.

use orchester_core::{
  id::ChatMessageId,
  orchestrator::{ActionOutcome, AgentWait, WaitKind},
  run::PendingToolApproval,
  store::RunStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{app_mcp::ToolError, orchestrator::tools::ChatScope};
use serde_json::{Value, json};

use super::{
  Served,
  orchestrator_chat::{actions, allow, chat, chat_call},
};

fn decide(request: &PendingToolApproval) -> Value {
  json!({
    "project": "Kontur",
    "approval": request.id,
    "decision": "approve_once",
    "reason": "The brief asks for it.",
  })
}

fn refused(called: &Result<Value, ToolError>) -> Option<&str> {
  match called {
    Err(ToolError::Refused { code, .. }) => Some(code),
    _ => None,
  }
}

/// The turn `request`'s wait started by itself in `scope`'s conversation, thinking now.
fn turn_of_request(db: &mut ProjectDb, scope: &ChatScope, request: &PendingToolApproval) {
  let run = db.run(request.run).expect("read").expect("the run");
  let wait = AgentWait {
    kind: WaitKind::Approval,
    task: "KON".to_owned(),
    title: "A tool request".to_owned(),
    run: run.id(),
    number: run.number().get(),
    step: None,
    place: None,
    others: 0,
    agent: Some(request.agent.clone()),
    link: "/project/p/tasks".to_owned(),
    automatic: true,
    continuing: None,
    merging: None,
  };
  let conversation = db.chat(scope.chat).expect("read").expect("the conversation");
  let key = format!("approval:{}:{}", run.id(), request.id);
  db.keep_agent_wait(&conversation, (&key, &wait), Timestamp::now())
    .expect("kept");
  let seq = db
    .automatic_waits_without_turn()
    .expect("waits")
    .into_iter()
    .find(|kept| kept.key == key)
    .expect("it waits for its turn")
    .seq;
  assert!(
    db.start_wait_turn((scope.chat, seq), ChatMessageId::generate())
      .expect("started")
  );
}

#[test]
fn orchestrator_approval_turn_decides_only_the_request_that_started_it() {
  let (mut served, seeded) = Served::seeded("cards");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let waiting = db.pending_tool_approvals_all().expect("waiting requests");
  let [first, second, ..] = waiting.as_slice() else {
    panic!("the cards wait on two requests at least: {waiting:?}");
  };
  // Only the drive switch: the chat's answers follow it, whatever Let automation answer approvals says.
  allow(&served, &seeded.repo);
  let person = chat(&served, &mut db);
  let own = ChatScope {
    on_its_own: true,
    ..person.clone()
  };
  turn_of_request(&mut db, &own, first);

  let other = chat_call(&mut served, &own, "decide_approval", decide(second));
  assert_eq!(refused(&other), Some("not_this_request"), "{other:?}");
  let kept = actions(&db, &own).pop().expect("kept");
  assert!(
    matches!(&kept.outcome, ActionOutcome::Refused { code, .. } if code == "not_this_request"),
    "{kept:?}"
  );

  // The chat's reason is required: the conversation shows it under the request.
  let mut unreasoned = decide(first);
  unreasoned.as_object_mut().expect("an object").remove("reason");
  let unreasoned = chat_call(&mut served, &own, "decide_approval", unreasoned);
  assert!(
    matches!(unreasoned, Err(ToolError::InvalidArguments(_))),
    "{unreasoned:?}"
  );
  // Its own request goes to the tool, as for any client (this session has no runner to deliver it), and the drive
  // switch is its consent.
  let own_request = chat_call(&mut served, &own, "decide_approval", decide(first));
  assert_ne!(refused(&own_request), Some("not_this_request"), "{own_request:?}");
  assert_ne!(refused(&own_request), Some("automation_not_allowed"), "{own_request:?}");
  let kept = actions(&db, &own).pop().expect("kept");
  assert!(kept.task.is_some(), "the decision names the request's task: {kept:?}");

  // A turn the person's message started may decide any request.
  let asked = chat_call(&mut served, &person, "decide_approval", decide(second));
  assert_ne!(refused(&asked), Some("not_this_request"), "{asked:?}");
}
