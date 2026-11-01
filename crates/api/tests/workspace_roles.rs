use orchester_api::v2::{
  InviteCreate, ManagedProject, MemberChanges, MemberCreate, MemberRole, Members, MembershipErrorCode,
  MembershipErrorDetails,
};
use orchester_core::id::{ProjectId, UserId};
use serde_json::{Value, json};

fn user() -> Value {
  json!({"id": UserId::generate(), "display_name": "Workspace admin"})
}

#[test]
fn workspace_roles_membership_requests_reject_owner_and_server_admin_escalation() {
  for role in ["admin", "member"] {
    let create: MemberCreate =
      serde_json::from_value(json!({"username":"alice", "role":role})).expect("valid membership");
    assert_eq!(serde_json::to_value(create).expect("serialize")["role"], role);
    let change: MemberChanges = serde_json::from_value(json!({"role":role})).expect("valid change");
    assert_eq!(serde_json::to_value(change).expect("serialize")["role"], role);
  }
  for invalid in ["owner", "server_admin", "viewer"] {
    assert!(serde_json::from_value::<MemberCreate>(json!({"username":"alice", "role":invalid})).is_err());
    assert!(serde_json::from_value::<MemberChanges>(json!({"role":invalid})).is_err());
  }
  assert!(serde_json::from_value::<MemberChanges>(json!({"role":"member", "admin":true})).is_err());
}

#[test]
fn workspace_roles_managed_projects_and_errors_expose_admins() {
  let admin = user();
  let project = json!({
    "id":ProjectId::generate(), "slug":"team", "address":"https://team.example/team",
    "key":"TEAM", "name":"Team", "remote_hint":null, "role":"admin", "next_number":1,
    "members":1, "open_invites":0, "admins":[admin], "created_at":1, "created_by":admin
  });
  let managed: ManagedProject = serde_json::from_value(project.clone()).expect("managed workspace");
  assert_eq!(managed.project.role, Some(MemberRole::Admin));
  assert_eq!(managed.admins.len(), 1);
  let wire = serde_json::to_value(managed).expect("serialize");
  assert!(wire.get("owners").is_none());
  assert_eq!(wire["admins"], project["admins"]);
  let mut legacy = project;
  legacy["owners"] = legacy
    .as_object_mut()
    .expect("object")
    .remove("admins")
    .expect("admins");
  assert!(serde_json::from_value::<ManagedProject>(legacy).is_err());
  let detail: MembershipErrorDetails = serde_json::from_value(json!({"admins":[admin]})).expect("admins");
  assert!(matches!(detail, MembershipErrorDetails::Admins { .. }));
  assert!(serde_json::from_value::<MembershipErrorDetails>(json!({"owners":[admin]})).is_err());
  assert_eq!(
    serde_json::to_value(MembershipErrorCode::LastWorkspaceAdmin).expect("code"),
    "last_workspace_admin"
  );
}

#[test]
fn workspace_roles_keep_server_admins_distinct_from_memberships() {
  let admin = user();
  let wire = json!({
    "members":[{"user":admin, "username":"alice", "role":"admin", "added_at":1}],
    "admins":[{"user":user(), "username":"server-admin"}]
  });
  let members: Members = serde_json::from_value(wire).expect("members and inherited server admins");
  assert_eq!(members.members[0].role, MemberRole::Admin);
  assert_eq!(members.admins.len(), 1);
  let roundtrip = serde_json::to_value(members).expect("serialize");
  assert!(roundtrip["admins"][0].get("role").is_none());
}

#[test]
fn workspace_roles_project_invites_use_the_shared_role_contract() {
  for role in ["admin", "member", "owner", "server_admin"] {
    let wire = json!({
      "kind":"project", "delivery":"code", "project":ProjectId::generate(), "role":role
    });
    let decoded = serde_json::from_value::<InviteCreate>(wire);
    if matches!(role, "admin" | "member") {
      let invite = decoded.expect("workspace invite");
      assert!(!invite.admin);
      assert_eq!(serde_json::to_value(invite).expect("serialize")["role"], role);
    } else {
      assert!(decoded.is_err());
    }
  }
}

#[test]
fn workspace_roles_legacy_contracts_remain_unchanged() {
  use orchester_api::{ErrorCode, MemberChanges, MemberCreate, MemberRole, SharingErrorDetails};

  for role in ["owner", "member"] {
    let request = json!({"username":"alice", "role":role});
    let create: MemberCreate = serde_json::from_value(request.clone()).expect("legacy request");
    assert_eq!(serde_json::to_value(create).expect("serialize"), request);
    let change: MemberChanges = serde_json::from_value(json!({"role":role})).expect("legacy role");
    assert_eq!(serde_json::to_value(change).expect("serialize")["role"], role);
  }
  assert!(serde_json::from_value::<MemberRole>(json!("admin")).is_err());
  assert_eq!(serde_json::to_value(MemberRole::Owner).expect("legacy owner"), "owner");
  assert_eq!(
    serde_json::to_value(ErrorCode::LastOwner).expect("legacy code"),
    "last_owner"
  );
  assert_eq!(
    serde_json::to_value(ErrorCode::LastAdmin).expect("server code"),
    "last_admin"
  );
  let contacts = json!({"owners":[user()]});
  let details: SharingErrorDetails = serde_json::from_value(contacts.clone()).expect("legacy contacts");
  assert!(matches!(details, SharingErrorDetails::Owners { .. }));
  assert_eq!(serde_json::to_value(details).expect("serialize"), contacts);
  let project = json!({
    "id":ProjectId::generate(), "slug":"team", "address":"https://team.example/team",
    "key":"TEAM", "name":"Team", "remote_hint":null, "role":"owner", "next_number":1,
    "members":1, "open_invites":0, "owners":[user()], "created_at":1, "created_by":user()
  });
  let managed: orchester_api::ManagedProject = serde_json::from_value(project.clone()).expect("legacy project");
  assert_eq!(managed.project.role, Some(MemberRole::Owner));
  assert_eq!(managed.owners.len(), 1);
  assert_eq!(serde_json::to_value(managed).expect("serialize"), project);
}

#[test]
fn workspace_roles_versioned_events_do_not_change_the_legacy_stream() {
  let delta = json!({
    "kind":"member.role_changed",
    "data":{"id":1, "by":user(), "actor":"person", "at":1, "user":UserId::generate(), "role":"admin"}
  });
  let event: orchester_api::v2::MembershipEvent = serde_json::from_value(delta.clone()).expect("v2 event");
  assert_eq!(serde_json::to_value(event).expect("serialize"), delta);
  assert!(serde_json::from_value::<orchester_api::Event>(delta.clone()).is_err());
  let mut legacy = delta;
  legacy["data"]["role"] = json!("owner");
  let event: orchester_api::Event = serde_json::from_value(legacy.clone()).expect("v1 event");
  assert_eq!(serde_json::to_value(event).expect("serialize"), legacy);
  assert!(serde_json::from_value::<orchester_api::v2::MembershipEvent>(legacy).is_err());
}
