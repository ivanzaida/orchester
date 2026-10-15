use lurq::app::events::{InputSource, input_source, with_synthetic_input};
use orchester_app::mcp::input_gate::{Guarded, InputGate};
use orchester_store::AppDb;
use orchester_workspace::{
  tasks::local_integration::{LocalIntent, LocalOrigin},
  workspace::Workspace,
};

#[test]
fn local_policy_queued_origin_scope_revocation_and_protection_counts_are_preserved() {
  let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-policy-tests");
  std::fs::create_dir_all(&scratch).expect("scratch");
  let dir = tempfile::tempdir_in(scratch).expect("private db");
  let workspace = Workspace::new(AppDb::open(&dir.path().join("app.db")).expect("db"));
  let gate = InputGate::default();
  gate.bind_permissions(workspace.clone());
  let off = with_synthetic_input(|| gate.local_input(input_source()));
  assert!(!off.recheck(), "no window scope");
  gate.bind_window_control(true);
  assert!(!off.recheck(), "later grant cannot authorize queued refused input");
  let ordinary = with_synthetic_input(|| gate.local_input(input_source()));
  assert_eq!(input_source(), InputSource::Os);
  assert_eq!(ordinary.origin(), LocalOrigin::WindowAutomation);
  assert!(ordinary.recheck(), "ordinary scope works with both opt-ins off");
  assert!(ordinary.origin().authorize(LocalIntent::Candidate).is_ok());
  assert!(ordinary.origin().authorize(LocalIntent::ReviewedDecision).is_err());
  workspace
    .set_automation_changes_permissions(true)
    .expect("private opt-in");
  workspace
    .set_automation_answers_approvals(true)
    .expect("private opt-in");
  let queued = with_synthetic_input(|| gate.local_input(input_source()));
  workspace.revoke_automation_permission_consent();
  workspace
    .set_automation_changes_permissions(true)
    .expect("later explicit grant");
  assert!(!queued.recheck(), "old queued action cannot borrow a later grant");
  let approval = with_synthetic_input(|| gate.local_input(input_source()));
  workspace.set_automation_answers_approvals(false).expect("revoke");
  workspace.set_automation_answers_approvals(true).expect("later grant");
  assert!(
    !approval.recheck(),
    "approval revocation also invalidates old queued input"
  );
  let queued = with_synthetic_input(|| gate.local_input(input_source()));
  let local = gate.protect(Guarded::LocalIntegrationQuestion);
  let secret = gate.protect(Guarded::WideningQuestion);
  drop(local);
  assert!(gate.protecting(), "dropping local protection cannot unlock another tab");
  assert!(!queued.recheck());
  drop(secret);
  assert!(!gate.protecting());
  assert!(
    !queued.recheck(),
    "a question that came and went invalidates queued input"
  );
  let fresh = with_synthetic_input(|| gate.local_input(input_source()));
  assert!(fresh.recheck());
  let settings = gate.protect(Guarded::PermissionsSetting);
  let native = gate.local_input(InputSource::Os);
  assert!(native.recheck(), "actual native origin retains its own authority");
  assert!(native.origin().authorize(LocalIntent::ReviewedDecision).is_ok());
  assert!(!fresh.recheck(), "both opt-ins never authorize Settings");
  drop(settings);
  gate.bind_window_control(false);
  assert!(!fresh.recheck(), "scope revocation");
}
