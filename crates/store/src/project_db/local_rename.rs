//! Renaming a local resource: its display name changes, its slug, its versions and its draft stay as they are (the
//! editors that save a draft with a name do the same, but a rename alone has no draft to save).

use orchester_core::{id::ResourceId, name::Name};
use rusqlite::params;

use super::{ProjectDb, resources::load_resources};
use crate::StoreError;

impl ProjectDb {
  /// Gives the local resource `id` the display name `name`; a resource the team shares is the team's to rename and is
  /// [`StoreError::ResourceNotFound`] here.
  pub fn rename_local_resource(&mut self, id: ResourceId, name: &Name) -> Result<(), StoreError> {
    let found = load_resources(
      &self.connection,
      "WHERE id = ?1 AND sharing IS NOT 'shared'",
      [id.to_string()],
    )?;
    if found.is_empty() {
      return Err(StoreError::ResourceNotFound(id));
    }
    self.connection.execute(
      "UPDATE resources SET name = ?1 WHERE id = ?2",
      params![name.as_str(), id.to_string()],
    )?;
    Ok(())
  }
}
