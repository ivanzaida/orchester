//! Who changes a shared project's configuration (`docs/architecture/server.md`, "Shared configuration", row 72): an
//! admin, the project's owner or the server's admin, on the team server itself
//! (`docs/design/team-configuration-live.md`, rule 6); a member's write of a team resource is refused here
//! ([`StoreError::NotAdmin`]). Nothing waits here to be sent: the versions of the team's resources in this database are
//! the copy the stream keeps, and an admin's change is sent to the server by the workspace.

use orchester_core::{
  id::ResourceId,
  resource::ResourceKind,
  sharing::{MemberRole, may_change_configuration},
};
use rusqlite::{Connection, OptionalExtension};

use super::{ProjectDb, resources::parse_kind, sharing::parse_role};
use crate::StoreError;

impl ProjectDb {
  /// Records whether the person signed in to the shared project's server from here is its admin.
  pub fn set_server_admin(&mut self, admin: bool) -> Result<(), StoreError> {
    let updated = self
      .connection
      .execute("UPDATE sharing SET server_admin = ?1", [admin])?;
    if updated == 0 {
      return Err(StoreError::NotShared);
    }
    Ok(())
  }

  /// Whether the person signed in to the shared project's server from here is its admin, as last recorded.
  pub fn server_admin(&self) -> Result<bool, StoreError> {
    self
      .connection
      .query_row("SELECT server_admin FROM sharing", [], |row| row.get(0))
      .optional()?
      .ok_or(StoreError::NotShared)
  }

  /// Whether the person may change the project's team configuration: always in a local project, and in a shared one
  /// when they are its owner or the server's admin.
  pub fn may_change_configuration(&self) -> Result<bool, StoreError> {
    Ok(configuring(&self.connection)?.unwrap_or(true))
  }

  /// Whether `resource` is the team's in a shared project; `false` in a local project and for what this computer keeps
  /// to itself (*Not shared*).
  pub fn is_team_resource(&self, resource: ResourceId) -> Result<bool, StoreError> {
    team_resource(&self.connection, resource)
  }
}

/// Whether the person may change a shared project's team configuration; `None` while the project is local.
fn configuring(connection: &Connection) -> Result<Option<bool>, StoreError> {
  let row: Option<(Option<String>, bool)> = connection
    .query_row("SELECT role, server_admin FROM sharing", [], |row| {
      Ok((row.get(0)?, row.get(1)?))
    })
    .optional()?;
  row
    .map(|(role, admin)| {
      let role: Option<MemberRole> = role.as_deref().map(parse_role).transpose()?;
      Ok(may_change_configuration(role, admin))
    })
    .transpose()
}

/// Whether resources of `kind` are the team's in a shared project: every kind but agent testing's.
pub(super) const fn shared_kind(kind: ResourceKind) -> bool {
  !matches!(
    kind,
    ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack
  )
}

/// Fails when a resource of `kind` would be the team's and the person may not change the team's configuration.
pub(super) fn ensure_may_create(connection: &Connection, kind: ResourceKind) -> Result<(), StoreError> {
  match configuring(connection)? {
    Some(false) if shared_kind(kind) => Err(StoreError::NotAdmin),
    _ => Ok(()),
  }
}

/// Fails when `resource` is the team's and the person may not change the team's configuration. A resource kept *Not
/// shared* when the project joined stays the person's own.
pub(super) fn ensure_may_change(connection: &Connection, resource: ResourceId) -> Result<(), StoreError> {
  if configuring(connection)? != Some(false) {
    return Ok(());
  }
  if team_resource(connection, resource)? {
    return Err(StoreError::NotAdmin);
  }
  Ok(())
}

/// Whether `resource` is the team's: the project is shared, the kind is shared and it was not kept *Not shared*.
pub(super) fn team_resource(connection: &Connection, resource: ResourceId) -> Result<bool, StoreError> {
  let shared: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM sharing)", [], |row| row.get(0))?;
  if !shared {
    return Ok(false);
  }
  let row: Option<(String, Option<String>)> = connection
    .query_row(
      "SELECT kind, sharing FROM resources WHERE id = ?1",
      [resource.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?;
  let (kind, sharing) = row.ok_or(StoreError::ResourceNotFound(resource))?;
  Ok(shared_kind(parse_kind(&kind)?) && sharing.as_deref() != Some("not_shared"))
}
