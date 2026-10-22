//! Drafts automation saved over Orchester's MCP server (ORC-30): a page marks them so the person sees who wrote a
//! draft before publishing it. A mark holds while the draft is still the one automation saved.

use std::collections::BTreeMap;

use orchester_core::{id::ResourceId, time::Timestamp};
use rusqlite::params;

use super::ProjectDb;
use crate::{StoreError, rows::parse};

impl ProjectDb {
  /// Records that automation saved the draft of `resource` at `saved_at`, its `updated_at`.
  pub fn mark_automation_draft(&mut self, resource: ResourceId, saved_at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO automation_drafts (resource_id, saved_at) VALUES (?1, ?2) \
       ON CONFLICT (resource_id) DO UPDATE SET saved_at = excluded.saved_at",
      params![resource.to_string(), saved_at.unix_millis()],
    )?;
    Ok(())
  }

  /// The resources whose draft is still the one automation saved (none saved since, none published), with when it
  /// saved it.
  pub fn automation_drafts(&self) -> Result<BTreeMap<ResourceId, Timestamp>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT marks.resource_id, marks.saved_at FROM automation_drafts AS marks \
       JOIN resource_drafts AS drafts ON drafts.resource_id = marks.resource_id AND drafts.updated_at = marks.saved_at",
    )?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .iter()
      .map(|(id, saved_at)| Ok((parse("resource id", id)?, Timestamp::from_unix_millis(*saved_at))))
      .collect()
  }
}
