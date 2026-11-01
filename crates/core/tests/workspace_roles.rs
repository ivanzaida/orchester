use orchester_core::sharing::{
  MemberRole, MembershipChangeError, check_workspace_admin_change, may_administer_workspace, may_change_configuration,
  may_read_workspace,
};

#[test]
fn workspace_roles_inherit_access_without_cross_workspace_membership() {
  for (role, server_admin, reads, administers) in [
    (None, false, false, false),
    (Some(MemberRole::Member), false, true, false),
    (Some(MemberRole::Admin), false, true, true),
    (None, true, true, true),
    (Some(MemberRole::Member), true, true, true),
    (Some(MemberRole::Admin), true, true, true),
  ] {
    assert_eq!(may_read_workspace(role, server_admin), reads);
    assert_eq!(may_administer_workspace(role, server_admin), administers);
    assert_eq!(may_change_configuration(role, server_admin), administers);
  }
  // An admin membership from another workspace is not the requested workspace's role.
  assert!(!may_administer_workspace(None, false));
}

#[test]
fn workspace_roles_preserve_the_last_workspace_admin() {
  for next in [None, Some(MemberRole::Member)] {
    for admins in [0, 1] {
      assert_eq!(
        check_workspace_admin_change(MemberRole::Admin, next, admins),
        Err(MembershipChangeError::LastAdmin)
      );
    }
    assert_eq!(check_workspace_admin_change(MemberRole::Admin, next, 2), Ok(()));
    assert_eq!(check_workspace_admin_change(MemberRole::Member, next, 1), Ok(()));
  }
  assert_eq!(
    check_workspace_admin_change(MemberRole::Admin, Some(MemberRole::Admin), 1),
    Ok(())
  );
  assert_eq!(
    check_workspace_admin_change(MemberRole::Member, Some(MemberRole::Admin), 1),
    Ok(())
  );
}

#[test]
fn workspace_roles_have_only_admin_and_member_wire_values() {
  for (role, wire) in [(MemberRole::Admin, "admin"), (MemberRole::Member, "member")] {
    assert_eq!(serde_json::to_value(role).expect("serialize role"), wire);
    assert_eq!(
      serde_json::from_value::<MemberRole>(wire.into()).expect("read role"),
      role
    );
  }
  for invalid in ["owner", "viewer", "server_admin", "Admin"] {
    assert!(serde_json::from_value::<MemberRole>(invalid.into()).is_err());
  }
}
