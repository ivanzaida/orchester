//! Which version of a resource this computer uses, and which number the next one takes.

use orchester_core::{
  id::ResourceId,
  resource::{ResourceBody, Version, VersionNumber},
};
use rusqlite::Connection;

use super::resources::{load_versions, version_number};
use crate::StoreError;

/// The version this computer uses of `resource`: its newest. Runs, editors, Always allow and publishing all start from
/// it.
pub(super) fn used_here<B: ResourceBody>(
  connection: &Connection,
  resource: ResourceId,
) -> Result<Option<Version<B>>, StoreError> {
  Ok(
    load_versions(
      connection,
      "WHERE resource_id = ?1 ORDER BY number DESC LIMIT 1",
      [resource.to_string()],
    )?
    .pop(),
  )
}

/// The number of the newest version this computer has of `resource`; the next version follows it.
pub(super) fn newest_number(
  connection: &Connection,
  resource: ResourceId,
) -> Result<Option<VersionNumber>, StoreError> {
  let number: Option<i64> = connection.query_row(
    "SELECT MAX(number) FROM resource_versions WHERE resource_id = ?1",
    [resource.to_string()],
    |row| row.get(0),
  )?;
  number.map(version_number).transpose()
}
