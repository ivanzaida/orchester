//! `project_default_pipeline`: the pipeline New task starts with (Pipelines › a row's *Make default*), one row; a
//! project without one starts New task with no pipeline chosen.

use orchester_core::id::ResourceId;
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{StoreError, rows::parse};

impl ProjectDb {
  /// The project's default pipeline, if a person chose one.
  pub fn default_pipeline(&self) -> Result<Option<ResourceId>, StoreError> {
    let id: Option<String> = self
      .connection
      .query_row("SELECT pipeline_id FROM project_default_pipeline", [], |row| row.get(0))
      .optional()?;
    id.map(|id| parse("default pipeline", &id)).transpose()
  }

  /// Makes `pipeline` the project's default, or none.
  pub fn save_default_pipeline(&mut self, pipeline: Option<ResourceId>) -> Result<(), StoreError> {
    match pipeline {
      Some(id) => self.connection.execute(
        "INSERT INTO project_default_pipeline (id, pipeline_id) VALUES (1, ?1) \
         ON CONFLICT (id) DO UPDATE SET pipeline_id = excluded.pipeline_id",
        params![id.to_string()],
      )?,
      None => self.connection.execute("DELETE FROM project_default_pipeline", [])?,
    };
    Ok(())
  }
}
