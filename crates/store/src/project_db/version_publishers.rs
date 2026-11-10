//! Who published each version of the team's resources this computer keeps (`docs/design/shared-configuration.md`,
//! decision 2), which a comparison of versions names.

use orchester_core::id::{ResourceId, VersionId};

use super::ProjectDb;
use crate::{StoreError, rows::parse};

impl ProjectDb {
  /// Who published each version of `resource` the server numbered, by its name on the server, oldest first; `None`
  /// for a version published here before the project was shared.
  pub fn version_publishers(&self, resource: ResourceId) -> Result<Vec<(VersionId, Option<String>)>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT id, publisher_name FROM resource_versions WHERE resource_id = ?1 ORDER BY number")?;
    let rows = statement
      .query_map([resource.to_string()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, publisher)| Ok((parse("version id", &id)?, publisher)))
      .collect()
  }
}
