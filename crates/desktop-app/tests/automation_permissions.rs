//! The window gate honors only verified consent and never broadens unrelated protected controls.
use orchester_app::mcp::input_gate::{Guarded, InputGate};
use orchester_store::AppDb;
use orchester_workspace::workspace::Workspace;

#[test]
fn automation_policy_unknown_off_on_and_revoked_are_distinct() {
  let dir = private_directory();
  let workspace = Workspace::new(AppDb::open(&dir.path().join("app.db")).expect("private database"));
  let gate = InputGate::default();
  gate.bind_permissions(workspace.clone());
  let _question = gate.protect(Guarded::PermissionWidening);
  assert!(gate.protecting(), "unknown cannot grant");
  assert!(
    !workspace
      .automation_changes_permissions()
      .expect("default consent read")
  );
  assert!(gate.protecting(), "off protects");
  workspace
    .set_automation_changes_permissions(true)
    .expect("actual stored opt-in");
  assert!(!gate.protecting(), "classified permission question honors on");
  workspace
    .set_automation_changes_permissions(false)
    .expect("actual revocation");
  assert!(gate.protecting(), "mounted question becomes protected again");
}

#[test]
fn automation_policy_settings_and_sensitive_questions_ignore_permission_opt_in() {
  let dir = private_directory();
  let workspace = Workspace::new(AppDb::open(&dir.path().join("app.db")).expect("private database"));
  workspace
    .set_automation_changes_permissions(true)
    .expect("stored permission opt-in");
  let gate = InputGate::default();
  gate.bind_permissions(workspace.clone());
  let _approval = gate.protect(Guarded::ApprovalWidening);
  assert!(
    gate.protecting(),
    "permission consent alone never grants automatic approvals"
  );
  workspace
    .set_automation_answers_approvals(true)
    .expect("separate actual approval opt-in");
  assert!(!gate.protecting());
  let setting = gate.protect(Guarded::PermissionsSetting);
  assert!(
    gate.protecting(),
    "automation cannot change its own opt-in even with both grants"
  );
  drop(setting);
  assert!(!gate.protecting());
  let secret = gate.protect(Guarded::WideningQuestion);
  assert!(
    gate.protecting(),
    "secret rotation or stale-base publication remain person-only"
  );
  drop(secret);
  assert!(!gate.protecting(), "dropping unrelated card retains classified consent");
  workspace
    .set_automation_answers_approvals(false)
    .expect("revoke automatic approvals");
  assert!(
    gate.protecting(),
    "separate approval revocation protects its mounted question"
  );
}

#[test]
fn automation_policy_failed_stored_read_cannot_reuse_a_prior_grant() {
  let dir = private_directory();
  let path = dir.path().join("app.db");
  let workspace = Workspace::new(AppDb::open(&path).expect("private database"));
  workspace
    .set_automation_changes_permissions(true)
    .expect("stored opt-in");
  let gate = InputGate::default();
  gate.bind_permissions(workspace.clone());
  let _question = gate.protect(Guarded::PermissionWidening);
  assert!(!gate.protecting());
  let db = rusqlite::Connection::open(path).expect("same private database");
  db.execute("DROP TABLE app_settings", [])
    .expect("real unreadable consent fixture");
  assert!(workspace.automation_changes_permissions().is_err());
  assert!(
    gate.protecting(),
    "failed authority read must not retain a successful grant"
  );
}

#[test]
fn automation_policy_an_answer_uses_the_shared_per_kind_grant() {
  use lurq::app::events::with_synthetic_input;
  use orchester_app::mcp::input_gate::answerer;
  use orchester_core::run::Answerer;

  let dir = private_directory();
  let workspace = Workspace::new(AppDb::open(&dir.path().join("app.db")).expect("private database"));
  let gate = InputGate::default();
  gate.bind_permissions(workspace.clone());
  let policy = Guarded::Decision(orchester_workspace::decisions::DecisionKind::Approval);
  assert_eq!(
    answerer(Some(&gate), policy),
    Some(Answerer::Person),
    "a person's own input answers as the person"
  );
  assert_eq!(
    with_synthetic_input(|| answerer(Some(&gate), policy)),
    None,
    "an MCP client's input answers nothing without consent"
  );
  workspace
    .set_automation_answers_approvals(true)
    .expect("the approvals opt-in");
  assert_eq!(
    with_synthetic_input(|| answerer(Some(&gate), policy)),
    Some(Answerer::Automation),
    "the approvals opt-in is sufficient for a decision, without permission setup consent"
  );
  workspace
    .set_automation_changes_permissions(true)
    .expect("the permissions opt-in");
  assert_eq!(
    with_synthetic_input(|| answerer(Some(&gate), policy)),
    Some(Answerer::Automation),
    "under consent an MCP client's input answers as automation, never as the person"
  );
  let setting = gate.protect(Guarded::PermissionsSetting);
  assert_eq!(
    with_synthetic_input(|| answerer(Some(&gate), policy)),
    None,
    "nothing while a person-only control is mounted"
  );
  drop(setting);
  assert_eq!(
    with_synthetic_input(|| answerer(None, policy)),
    None,
    "no gate, no consent"
  );
  assert_eq!(answerer(None, policy), Some(Answerer::Person));
}

#[test]
fn local_integration_decisions_remain_person_only_with_all_automation_grants() {
  let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
  std::fs::create_dir_all(&scratch).expect("scratch");
  let dir = tempfile::tempdir_in(scratch).expect("private policy directory");
  let workspace = Workspace::new(AppDb::open(&dir.path().join("app.db")).expect("database"));
  workspace
    .set_automation_changes_permissions(true)
    .expect("permission consent");
  workspace
    .set_automation_answers_approvals(true)
    .expect("approval consent");
  let gate = InputGate::default();
  gate.bind_permissions(workspace);
  for master_control in [Guarded::Accounts, Guarded::McpServer] {
    let master = gate.protect(master_control);
    let local = gate.protect(Guarded::LocalIntegrationQuestion);
    assert!(gate.protecting());
    assert!(!gate.widening_allowed(Guarded::LocalIntegrationQuestion));
    assert!(!gate.widening_allowed(master_control));
    drop(local);
    assert!(gate.protecting(), "local release cannot unlock a master control");
    drop(master);
    assert!(!gate.protecting());
  }
}
fn private_directory() -> tempfile::TempDir {
  let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-policy-tests");
  std::fs::create_dir_all(&scratch).expect("private worktree scratch");
  tempfile::tempdir_in(scratch).expect("private policy directory")
}
