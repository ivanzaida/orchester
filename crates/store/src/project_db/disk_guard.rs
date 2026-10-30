//! `disk_guard` (ORC-422), one row: the free space the disk guard keeps, written only when it is set; a project
//! without the row keeps [`KeepFree::DEFAULT`].

use orchester_core::workspaces::KeepFree;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{StoreError, rows::corrupt};

pub(super) fn load(connection: &Connection) -> Result<KeepFree, StoreError> {
  let gigabytes: Option<i64> = connection
    .query_row("SELECT keep_free_gb FROM disk_guard", [], |row| row.get(0))
    .optional()?;
  let Some(gigabytes) = gigabytes else {
    return Ok(KeepFree::DEFAULT);
  };
  u64::try_from(gigabytes)
    .ok()
    .and_then(|gigabytes| KeepFree::from_gigabytes(gigabytes).ok())
    .ok_or_else(|| corrupt("keep free", gigabytes))
}

pub(super) fn save(connection: &Connection, keep: KeepFree) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO disk_guard (id, keep_free_gb) VALUES (1, ?1) \
     ON CONFLICT (id) DO UPDATE SET keep_free_gb = excluded.keep_free_gb",
    params![i64::try_from(keep.gigabytes()).map_err(|_| StoreError::Unrepresentable("keep free"))?],
  )?;
  Ok(())
}
