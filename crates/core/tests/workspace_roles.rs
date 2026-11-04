use orchester_core::sharing::{
  MembershipChangeError, WorkspaceRole, check_workspace_admin_change, may_administer_workspace, may_read_workspace,
};

#[test]
fn workspace_roles_inherit_access_without_cross_workspace_membership() {
  for (role, server_admin, reads, administers) in [
    (None, false, false, false),
    (Some(WorkspaceRole::Member), false, true, false),
    (Some(WorkspaceRole::Admin), false, true, true),
    (None, true, true, true),
    (Some(WorkspaceRole::Member), true, true, true),
    (Some(WorkspaceRole::Admin), true, true, true),
  ] {
    assert_eq!(may_read_workspace(role, server_admin), reads);
    assert_eq!(may_administer_workspace(role, server_admin), administers);
  }
  // An admin membership from another workspace is not the requested workspace's role.
  assert!(!may_administer_workspace(None, false));
}

#[test]
fn workspace_roles_preserve_the_last_workspace_admin() {
  for next in [None, Some(WorkspaceRole::Member)] {
    for admins in [0, 1] {
      assert_eq!(
        check_workspace_admin_change(WorkspaceRole::Admin, next, admins),
        Err(MembershipChangeError::LastAdmin)
      );
    }
    assert_eq!(check_workspace_admin_change(WorkspaceRole::Admin, next, 2), Ok(()));
    assert_eq!(check_workspace_admin_change(WorkspaceRole::Member, next, 1), Ok(()));
  }
  assert_eq!(
    check_workspace_admin_change(WorkspaceRole::Admin, Some(WorkspaceRole::Admin), 1),
    Ok(())
  );
  assert_eq!(
    check_workspace_admin_change(WorkspaceRole::Member, Some(WorkspaceRole::Admin), 1),
    Ok(())
  );
}

#[test]
fn workspace_roles_have_only_admin_and_member_wire_values() {
  for (role, wire) in [(WorkspaceRole::Admin, "admin"), (WorkspaceRole::Member, "member")] {
    assert_eq!(serde_json::to_value(role).expect("serialize role"), wire);
    assert_eq!(
      serde_json::from_value::<WorkspaceRole>(wire.into()).expect("read role"),
      role
    );
  }
  for invalid in ["owner", "viewer", "server_admin", "Admin"] {
    assert!(serde_json::from_value::<WorkspaceRole>(invalid.into()).is_err());
  }
}

#[test]
fn workspace_roles_adapt_legacy_memberships_without_changing_them() {
  use orchester_core::sharing::{MemberRole, may_change_configuration};

  for (legacy, workspace, wire) in [
    (MemberRole::Owner, WorkspaceRole::Admin, "owner"),
    (MemberRole::Member, WorkspaceRole::Member, "member"),
  ] {
    assert_eq!(WorkspaceRole::from(legacy), workspace);
    assert_eq!(MemberRole::from(workspace), legacy);
    assert_eq!(serde_json::to_value(legacy).expect("legacy wire"), wire);
    for server_admin in [false, true] {
      assert_eq!(
        may_change_configuration(Some(legacy), server_admin),
        may_administer_workspace(Some(workspace), server_admin)
      );
    }
  }
}
