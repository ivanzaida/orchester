//! How each of the team's resources stands on this computer (`docs/design/shared-configuration.md`, decisions 2, 4
//! and 5): the latest version the server numbered, with its publisher, and the versions an admin published here that
//! wait for the server.

use orchester_core::{
  id::{ResourceId, VersionId},
  resource::{ResourceKind, VersionNumber},
  time::Timestamp,
};

use super::{
  ProjectDb,
  resources::{parse_kind, version_number},
  shared_publishes::{PublishState, shared_kind},
};
use crate::{StoreError, rows::parse};

/// The latest version of a team resource the server numbered, or that was published here before the project was
/// shared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmedVersion {
  pub number: VersionNumber,
  pub published_at: Timestamp,
  /// Who published it, by their name on the server; `None` for a version published here before the project was shared.
  pub publisher: Option<String>,
}

/// A version published here that the server has not numbered yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingVersion {
  pub version: VersionId,
  pub published_at: Timestamp,
  /// The number of the version it was edited from; `None` for a resource's first version.
  pub based_on: Option<VersionNumber>,
  pub state: PublishState,
}

/// How one of the team's resources stands on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedStanding {
  pub resource: ResourceId,
  pub kind: ResourceKind,
  pub confirmed: Option<ConfirmedVersion>,
  /// The versions published here that wait for the server, oldest first.
  pub waiting: Vec<WaitingVersion>,
}

impl ProjectDb {
  /// How each of the team's resources stands, in the order they were added; none while the project is local. A
  /// resource kept *Not shared* when the project joined, and agent testing's, are the person's own and not listed.
  pub fn shared_standings(&self) -> Result<Vec<SharedStanding>, StoreError> {
    if self.sharing()?.is_none() {
      return Ok(Vec::new());
    }
    let mut statement = self.connection.prepare(
      "SELECT resources.id, resources.kind, latest.number, latest.published_at, latest.publisher_name \
       FROM resources LEFT JOIN resource_versions AS latest ON latest.id = \
       (SELECT id FROM resource_versions WHERE resource_id = resources.id \
        AND id NOT IN (SELECT version_id FROM shared_publishes) ORDER BY number DESC LIMIT 1) \
       WHERE resources.sharing IS NULL OR resources.sharing <> 'not_shared' ORDER BY resources.rowid",
    )?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, Option<i64>>(2)?,
          row.get::<_, Option<i64>>(3)?,
          row.get::<_, Option<String>>(4)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    let waiting = self.waiting_publishes()?;
    let mut standings = Vec::new();
    for (resource, kind, number, published_at, publisher) in rows {
      let kind = parse_kind(&kind)?;
      if !shared_kind(kind) {
        continue;
      }
      let resource: ResourceId = parse("resource id", &resource)?;
      let confirmed = match (number, published_at) {
        (Some(number), Some(published_at)) => Some(ConfirmedVersion {
          number: version_number(number)?,
          published_at: Timestamp::from_unix_millis(published_at),
          publisher,
        }),
        _ => None,
      };
      let waiting = waiting
        .iter()
        .filter(|publish| publish.resource == resource)
        .map(|publish| {
          Ok(WaitingVersion {
            version: publish.version,
            published_at: self.published_at(publish.version)?,
            based_on: publish.based_on,
            state: publish.state.clone(),
          })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
      standings.push(SharedStanding {
        resource,
        kind,
        confirmed,
        waiting,
      });
    }
    Ok(standings)
  }

  fn published_at(&self, version: VersionId) -> Result<Timestamp, StoreError> {
    let at: i64 = self.connection.query_row(
      "SELECT published_at FROM resource_versions WHERE id = ?1",
      [version.to_string()],
      |row| row.get(0),
    )?;
    Ok(Timestamp::from_unix_millis(at))
  }
}
