//! The chat's git tools through the chat's own entry point (`orchestrator::tools::call`), on the seeded Kontur: a write
//! is refused while *Let the Orchestrator write with git* is off, and with it on, still refused in a turn a wait or a
//! run's end started by itself (`person_only`) and in a day's report's turn (`report_turn`), each refusal kept in the
//! conversation; what can lose work is a card in any turn; a read is open and not kept.

use orchester_core::orchestrator::{ActionOutcome, change::Change};
use orchester_store::ProjectDb;
use orchester_workspace::{app_mcp::ToolError, orchestrator::tools::ChatScope, workspace::OrchestratorConsent};
use serde_json::json;

use super::{
  Served,
  orchestrator_chat::{actions, allow, chat, chat_call},
  orchestrator_report_turn::report_turn,
};

fn refusal(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    other => panic!("a refusal: {other:?}"),
  }
}

/// The code of the newest action `scope`'s conversation kept, when it was refused.
fn kept_refusal(db: &ProjectDb, scope: &ChatScope) -> String {
  match actions(db, scope).pop().expect("kept").outcome {
    ActionOutcome::Refused { code, .. } => code,
    other => panic!("a kept refusal: {other:?}"),
  }
}

#[test]
fn chat_git_writes_wait_for_the_switch_and_never_run_in_a_turn_nobody_asked_for() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  let commit = json!({ "project": "Kontur", "message": "From the chat", "all": true });
  let off = chat_call(&mut served, &scope, "git_commit", commit.clone()).expect_err("refused");
  assert_eq!(refusal(&off), "write_off");
  assert_eq!(kept_refusal(&db, &scope), "write_off");

  let read = chat_call(&mut served, &scope, "git_status", json!({ "project": "Kontur" })).expect("read");
  assert!(read["branch"].is_string(), "{read}");
  let kept = actions(&db, &scope).len();

  allow(&served, &seeded.repo, OrchestratorConsent::WriteGit);
  let on_its_own = ChatScope {
    on_its_own: true,
    ..scope.clone()
  };
  let refused = chat_call(&mut served, &on_its_own, "git_commit", commit).expect_err("refused");
  assert_eq!(refusal(&refused), "person_only");
  assert_eq!(kept_refusal(&db, &scope), "person_only");
  assert_eq!(
    actions(&db, &scope).len(),
    kept + 1,
    "the read kept nothing, the refusal is kept"
  );

  let report = ChatScope {
    chat: report_turn(&mut db, &scope),
    on_its_own: true,
    ..scope.clone()
  };
  let refused = chat_call(
    &mut served,
    &report,
    "git_tag",
    json!({ "project": "Kontur", "name": "v9" }),
  )
  .expect_err("refused");
  assert_eq!(refusal(&refused), "report_turn");
  assert_eq!(kept_refusal(&db, &report), "report_turn");

  let reset = json!({ "project": "Kontur", "operation": { "kind": "reset", "to": "HEAD", "mode": "hard" } });
  let told = chat_call(&mut served, &on_its_own, "git_propose", reset).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  let proposed = actions(&db, &scope).pop().expect("kept");
  assert_eq!(proposed.outcome, ActionOutcome::Waits);
  assert!(matches!(
    proposed.change.as_deref().map(|change| &change.change),
    Some(Change::Git(_))
  ));
}
