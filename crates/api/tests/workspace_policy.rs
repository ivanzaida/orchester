use orchester_api::{
  ServerSettings, SettingsChanges,
  v2::{
    MemberRole, PolicyRevision, WorkspaceCapabilities, WorkspacePolicy, WorkspacePolicyChanges, WorkspacePolicyError,
    WorkspacePolicyResponse,
  },
};
use serde_json::json;

#[test]
fn workspace_policy_server_threshold_roundtrip_and_validation() {
  let legacy: ServerSettings = serde_json::from_value(json!({"name":"Team", "join_approval":true})).expect("legacy");
  assert_eq!(legacy.claim_staleness_seconds.seconds(), 900);
  for seconds in [300, 900, 86400] {
    let wire = json!({"name":"Team", "join_approval":true, "claim_staleness_seconds":seconds});
    let settings: ServerSettings = serde_json::from_value(wire.clone()).expect("settings");
    assert_eq!(serde_json::to_value(settings).expect("serialize"), wire);
    let wire = json!({"name":null, "join_approval":null, "claim_staleness_seconds":seconds});
    let change: SettingsChanges = serde_json::from_value(wire.clone()).expect("change");
    assert_eq!(serde_json::to_value(change).expect("serialize"), wire);
  }
  let unchanged: SettingsChanges = serde_json::from_value(json!({})).expect("no changes");
  assert!(unchanged.claim_staleness_seconds.is_none());
  for invalid in [json!(299), json!(86401), json!(-1), json!(300.5), json!("900")] {
    assert!(serde_json::from_value::<SettingsChanges>(json!({"claim_staleness_seconds":invalid})).is_err());
    assert!(
      serde_json::from_value::<ServerSettings>(json!({"join_approval":true,"claim_staleness_seconds":invalid}))
        .is_err()
    );
  }
  assert!(serde_json::from_value::<SettingsChanges>(json!({"automatic_merge":true})).is_err());
}

#[test]
fn workspace_policy_read_change_and_default_roundtrip() {
  let policy = WorkspacePolicy::default();
  assert!(!policy.automatic_merge);
  assert_eq!(policy.revision, PolicyRevision::FIRST);
  let response = WorkspacePolicyResponse {
    policy,
    capabilities: WorkspaceCapabilities::for_caller(Some(MemberRole::Member), false),
  };
  let wire = json!({"policy":{"revision":1,"automatic_merge":false},
    "capabilities":{"read_policy":true,"change_policy":false}});
  assert_eq!(serde_json::to_value(response).expect("serialize"), wire);
  assert_eq!(
    serde_json::from_value::<WorkspacePolicyResponse>(wire).expect("read"),
    response
  );
  let wire = json!({"revision":1,"automatic_merge":true});
  let change: WorkspacePolicyChanges = serde_json::from_value(wire.clone()).expect("change");
  assert_eq!(serde_json::to_value(change).expect("serialize"), wire);
  for field in ["account", "claim_staleness_seconds", "default_pipeline", "capabilities"] {
    let mut invalid = wire.clone();
    invalid[field] = json!(true);
    assert!(serde_json::from_value::<WorkspacePolicyChanges>(invalid).is_err());
  }
  for invalid in [
    json!({"automatic_merge":true}),
    json!({"revision":1}),
    json!({"revision":0,"automatic_merge":true}),
  ] {
    assert!(serde_json::from_value::<WorkspacePolicyChanges>(invalid).is_err());
  }
}

#[test]
fn workspace_policy_capabilities_follow_workspace_and_inherited_server_authority() {
  for (role, admin, read, change) in [
    (None, false, false, false),
    (Some(MemberRole::Member), false, true, false),
    (Some(MemberRole::Admin), false, true, true),
    (None, true, true, true),
    (Some(MemberRole::Member), true, true, true),
  ] {
    assert_eq!(
      WorkspaceCapabilities::for_caller(role, admin),
      WorkspaceCapabilities {
        read_policy: read,
        change_policy: change
      }
    );
  }
}

#[test]
fn workspace_policy_revision_conflict_shape_and_revision_bounds() {
  let current = WorkspacePolicy {
    revision: PolicyRevision::FIRST.next().expect("next"),
    automatic_merge: true,
  };
  let conflict = WorkspacePolicyError::PolicyRevisionConflict { current };
  let wire = json!({"error":"policy_revision_conflict","current":{"revision":2,"automatic_merge":true}});
  assert_eq!(serde_json::to_value(conflict).expect("serialize"), wire);
  assert_eq!(
    serde_json::from_value::<WorkspacePolicyError>(wire).expect("conflict"),
    conflict
  );
  assert!(serde_json::from_value::<WorkspacePolicyError>(json!({"error":"policy_revision_conflict"})).is_err());
  assert!(
    serde_json::from_value::<WorkspacePolicyError>(json!({"error":"revision_conflict","current":current})).is_err()
  );
  assert!(PolicyRevision::try_from(0).is_err());
  assert!(PolicyRevision::try_from(u64::MAX).is_err());
  assert!(PolicyRevision::try_from(i64::MAX as u64).expect("max").next().is_err());
}
