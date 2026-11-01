use orchester_api::{
  ErrorCode, InviteCreate, ManagedProject, MemberChanges, MemberCreate, MemberRole, Members, SharingErrorDetails,
  TEAM_API_VERSION,
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
  let detail: SharingErrorDetails = serde_json::from_value(json!({"admins":[admin]})).expect("admins");
  assert!(matches!(detail, SharingErrorDetails::Admins { .. }));
  assert!(serde_json::from_value::<SharingErrorDetails>(json!({"owners":[admin]})).is_err());
  assert_eq!(
    serde_json::to_value(ErrorCode::LastWorkspaceAdmin).expect("code"),
    "last_workspace_admin"
  );
  assert_eq!(serde_json::to_value(ErrorCode::LastAdmin).expect("code"), "last_admin");
  assert_eq!(TEAM_API_VERSION, 4);
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
