//! Publishing a shared project's configuration from this computer (`docs/architecture/server.md`, "Shared
//! configuration", rows 36 and 72). Only an admin, the project's owner or the server's admin, changes the team's
//! resources: every write of a draft, a publish, a creation, an import or a removal refuses a member
//! ([`StoreError::NotAdmin`]). A version an admin publishes is kept at once with a number of its own here, which their
//! new runs use, and waits in `shared_publishes` until the server numbers it ([`ProjectDb::confirm_publish`]); a
//! teammate's version cached meanwhile takes its number and moves it up.

use orchester_core::{
  id::{IdempotencyKey, ResourceId, VersionId},
  resource::{ResourceBody, ResourceKind, Version, VersionNumber},
  sharing::{MemberRole, may_change_configuration},
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{
  ProjectDb,
  resources::{check_kind, parse_kind, version_number},
  shared_versions::CachedVersion,
  sharing::parse_role,
};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// Where a version published here stands with the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishState {
  /// It is sent when the server can be reached.
  Waiting,
  /// Someone published `latest` from the same base meanwhile: the admin decides ([`ProjectDb::publish_again`]).
  Conflict { latest: VersionNumber },
  /// The server refused it with its error `code`; it is not sent again on its own.
  Refused { code: String },
}

/// A version published here that the server has not numbered yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingPublish {
  pub version: VersionId,
  pub resource: ResourceId,
  pub kind: ResourceKind,
  /// Its number on this computer until the server gives it one.
  pub number: VersionNumber,
  pub key: IdempotencyKey,
  /// The number of the version it was edited from; `None` for a resource's first version.
  pub based_on: Option<VersionNumber>,
  pub state: PublishState,
}

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

  /// The versions published here that the server has not numbered, by resource, oldest first.
  pub fn waiting_publishes(&self) -> Result<Vec<WaitingPublish>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT shared_publishes.version_id, shared_publishes.resource_id, resources.kind, mine.number, \
       shared_publishes.idempotency_key, base.number, shared_publishes.conflict_latest, shared_publishes.refused \
       FROM shared_publishes \
       JOIN resource_versions AS mine ON mine.id = shared_publishes.version_id \
       JOIN resources ON resources.id = shared_publishes.resource_id \
       LEFT JOIN resource_versions AS base ON base.id = shared_publishes.base_version_id \
       ORDER BY resources.rowid, mine.number",
    )?;
    let rows = statement
      .query_map([], |row| {
        Ok(WaitingRow {
          version: row.get(0)?,
          resource: row.get(1)?,
          kind: row.get(2)?,
          number: row.get(3)?,
          key: row.get(4)?,
          based_on: row.get(5)?,
          conflict: row.get(6)?,
          refused: row.get(7)?,
        })
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(WaitingRow::into_publish).collect()
  }

  /// The server answered that `latest` was published from the same base as `version` meanwhile.
  pub fn record_publish_conflict(&mut self, version: VersionId, latest: VersionNumber) -> Result<(), StoreError> {
    self.set_publish_state(version, Some(latest.get()), None)
  }

  /// The server refused `version` with its error `code`.
  pub fn record_publish_refused(&mut self, version: VersionId, code: &str) -> Result<(), StoreError> {
    self.set_publish_state(version, None, Some(code))
  }

  /// The admin publishes `version` anyway, as the next version after the latest the server numbered: it waits to be
  /// sent again, based on that one.
  pub fn publish_again(&mut self, version: VersionId) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE shared_publishes SET conflict_latest = NULL, refused = NULL, base_version_id = \
       (SELECT id FROM resource_versions WHERE resource_id = shared_publishes.resource_id \
        AND id NOT IN (SELECT version_id FROM shared_publishes) ORDER BY number DESC LIMIT 1) \
       WHERE version_id = ?1",
      [version.to_string()],
    )?;
    if updated == 0 {
      return Err(corrupt("waiting publish", version));
    }
    Ok(())
  }

  /// The server numbered `version`, published here, as `confirmed`: it takes the server's number, moving up another
  /// waiting version that has it, and is kept as every version cached from the server is, with its publisher.
  pub fn confirm_publish<B: ResourceBody>(
    &mut self,
    version: VersionId,
    confirmed: &CachedVersion<B>,
  ) -> Result<Version<B>, StoreError> {
    let transaction = self.connection.transaction()?;
    let (resource, number): (String, i64) = transaction
      .query_row(
        "SELECT resource_versions.resource_id, resource_versions.number FROM shared_publishes \
         JOIN resource_versions ON resource_versions.id = shared_publishes.version_id \
         WHERE shared_publishes.version_id = ?1",
        [version.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?
      .ok_or_else(|| corrupt("waiting publish", version))?;
    let resource: ResourceId = parse("resource id", &resource)?;
    check_kind::<B>(&transaction, resource)?;
    if version_number(number)? != confirmed.number {
      make_room(&transaction, resource, confirmed.number)?;
      transaction.execute(
        "UPDATE resource_versions SET number = ?2 WHERE id = ?1",
        params![version.to_string(), confirmed.number.get()],
      )?;
    }
    transaction.execute(
      "DELETE FROM shared_publishes WHERE version_id = ?1",
      [version.to_string()],
    )?;
    let kept = super::shared_versions::keep_version(&transaction, resource, confirmed)?;
    // A version published here never waits for *Use v<N> here*, and supersedes a teammate's that waits below it.
    super::shared_versions::stop_waiting(&transaction, resource, confirmed.number)?;
    transaction.commit()?;
    Ok(kept)
  }

  fn set_publish_state(
    &mut self,
    version: VersionId,
    conflict: Option<u32>,
    refused: Option<&str>,
  ) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE shared_publishes SET conflict_latest = ?2, refused = ?3 WHERE version_id = ?1",
      params![version.to_string(), conflict, refused],
    )?;
    if updated == 0 {
      return Err(corrupt("waiting publish", version));
    }
    Ok(())
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
fn team_resource(connection: &Connection, resource: ResourceId) -> Result<bool, StoreError> {
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

/// Records that `version` of the team's `resource`, just published here from `based_on`, waits for the server.
pub(super) fn record<B>(
  connection: &Connection,
  resource: ResourceId,
  version: &Version<B>,
  based_on: Option<VersionNumber>,
) -> Result<(), StoreError> {
  if !team_resource(connection, resource)? {
    return Ok(());
  }
  let base: Option<String> = match based_on {
    Some(number) => Some(connection.query_row(
      "SELECT id FROM resource_versions WHERE resource_id = ?1 AND number = ?2",
      params![resource.to_string(), number.get()],
      |row| row.get(0),
    )?),
    None => None,
  };
  connection.execute(
    "INSERT INTO shared_publishes (version_id, resource_id, idempotency_key, base_version_id) VALUES (?1, ?2, ?3, ?4)",
    params![
      version.id().to_string(),
      resource.to_string(),
      IdempotencyKey::generate().to_string(),
      base
    ],
  )?;
  Ok(())
}

/// Moves every version of `resource` published here and waiting at `number` or above one number up, the highest
/// first, so a version the server numbered `number` can take it.
pub(super) fn make_room(
  connection: &Connection,
  resource: ResourceId,
  number: VersionNumber,
) -> Result<(), StoreError> {
  let mut statement = connection.prepare(
    "SELECT id FROM resource_versions WHERE resource_id = ?1 AND number >= ?2 \
     AND id IN (SELECT version_id FROM shared_publishes) ORDER BY number DESC",
  )?;
  let waiting = statement
    .query_map(params![resource.to_string(), number.get()], |row| {
      row.get::<_, String>(0)
    })?
    .collect::<Result<Vec<_>, _>>()?;
  for id in waiting {
    connection.execute("UPDATE resource_versions SET number = number + 1 WHERE id = ?1", [id])?;
  }
  Ok(())
}

/// The latest version of `resource` the server numbered, or that was published here before the project was shared.
pub(super) fn latest_confirmed(connection: &Connection, resource: ResourceId) -> Result<Option<i64>, StoreError> {
  Ok(connection.query_row(
    "SELECT MAX(number) FROM resource_versions WHERE resource_id = ?1 \
     AND id NOT IN (SELECT version_id FROM shared_publishes)",
    [resource.to_string()],
    |row| row.get(0),
  )?)
}

struct WaitingRow {
  version: String,
  resource: String,
  kind: String,
  number: i64,
  key: String,
  based_on: Option<i64>,
  conflict: Option<i64>,
  refused: Option<String>,
}

impl WaitingRow {
  fn into_publish(self) -> Result<WaitingPublish, StoreError> {
    let state = match (self.conflict, self.refused) {
      (None, None) => PublishState::Waiting,
      (Some(latest), None) => PublishState::Conflict {
        latest: version_number(latest)?,
      },
      (None, Some(code)) => PublishState::Refused { code },
      (Some(_), Some(_)) => return Err(corrupt("waiting publish", self.version)),
    };
    Ok(WaitingPublish {
      version: parse("version id", &self.version)?,
      resource: parse("resource id", &self.resource)?,
      kind: parse_kind(&self.kind)?,
      number: version_number(self.number)?,
      key: parse("idempotency key", &self.key)?,
      based_on: self.based_on.map(version_number).transpose()?,
      state,
    })
  }
}
