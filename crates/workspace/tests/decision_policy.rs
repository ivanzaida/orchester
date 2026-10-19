//! Every transport uses the same decision-kind consent; permission setup is a separate grant.
use orchester_store::AppDb;
use orchester_workspace::{decisions::DecisionKind, workspace::Workspace};

#[test]
fn decision_policy_each_kind_requires_approvals_and_honors_revocation() {
  let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-31-policy");
  std::fs::create_dir_all(&scratch).expect("scratch");
  let dir = tempfile::tempdir_in(scratch).expect("directory");
  let workspace = Workspace::new(AppDb::open(&dir.path().join("app.db")).expect("database"));
  let kinds = [
    DecisionKind::Approval,
    DecisionKind::Question,
    DecisionKind::Plan,
    DecisionKind::Stuck,
  ];
  for permissions in [false, true] {
    workspace
      .set_automation_changes_permissions(permissions)
      .expect("permission setup consent");
    for approvals in [false, true, false] {
      workspace
        .set_automation_answers_approvals(approvals)
        .expect("decision consent");
      for kind in kinds {
        assert_eq!(kind.read_consent(&workspace).expect("stored"), approvals, "{kind:?}");
        assert_eq!(kind.consent(&workspace), approvals, "{kind:?}");
        assert_eq!(kind.require(&workspace).is_ok(), approvals, "{kind:?}");
      }
    }
  }
}
