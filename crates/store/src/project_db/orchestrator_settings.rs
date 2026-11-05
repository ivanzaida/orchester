//! What the project's Orchestrator chat runs on (`orchester_core::orchestrator::OrchestratorSettings`): its published
//! versions and the draft saved since the latest.

use orchester_core::{orchestrator::OrchestratorSettings, resource::ModelId, time::Timestamp};
use rusqlite::{OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ProjectDb {
  /// What the Orchestrator runs on: its latest published version; `None` until a person published one.
  pub fn orchestrator_settings(&self) -> Result<Option<OrchestratorSettings>, StoreError> {
    Ok(self.orchestrator_published()?.map(|(_, settings)| settings))
  }

  /// The latest published version of what the Orchestrator runs on, with its number.
  pub fn orchestrator_published(&self) -> Result<Option<(u32, OrchestratorSettings)>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT version, connection_id, model, effort FROM orchestrator_versions ORDER BY version DESC LIMIT 1",
        [],
        |row| Ok((row.get::<_, i64>(0)?, settings_row(row, 1)?)),
      )
      .optional()?;
    row
      .map(|(version, raw)| {
        let version = u32::try_from(version).map_err(|error| corrupt("orchestrator version", error))?;
        Ok((version, raw.into_settings()?))
      })
      .transpose()
  }

  /// The draft saved since the latest published version; `None` when there is none.
  pub fn orchestrator_draft(&self) -> Result<Option<OrchestratorSettings>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT connection_id, model, effort FROM orchestrator_draft WHERE id = 1",
        [],
        |row| settings_row(row, 0),
      )
      .optional()?
      .map(RawSettings::into_settings)
      .transpose()
  }

  /// Saves `settings` as the draft, at `at`, in place of the one saved before.
  pub fn save_orchestrator_draft(&mut self, settings: &OrchestratorSettings, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO orchestrator_draft (id, connection_id, model, effort, saved_at) VALUES (1, ?1, ?2, ?3, ?4)
       ON CONFLICT (id) DO UPDATE SET connection_id = excluded.connection_id, model = excluded.model,
         effort = excluded.effort, saved_at = excluded.saved_at",
      params![
        settings.connection().to_string(),
        settings.model().as_str(),
        settings.effort(),
        at.unix_millis()
      ],
    )?;
    Ok(())
  }

  /// Publishes `settings` at `at` as the next version, which the chat answers on from its next reply; the draft is
  /// done with. Returns the version's number.
  pub fn publish_orchestrator(&mut self, settings: &OrchestratorSettings, at: Timestamp) -> Result<u32, StoreError> {
    let transaction = self.connection.transaction()?;
    let latest: i64 = transaction.query_row(
      "SELECT COALESCE(MAX(version), 0) FROM orchestrator_versions",
      [],
      |row| row.get(0),
    )?;
    let version = latest + 1;
    transaction.execute(
      "INSERT INTO orchestrator_versions (version, connection_id, model, effort, published_at)
       VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        version,
        settings.connection().to_string(),
        settings.model().as_str(),
        settings.effort(),
        at.unix_millis()
      ],
    )?;
    transaction.execute("DELETE FROM orchestrator_draft", [])?;
    transaction.commit()?;
    u32::try_from(version).map_err(|error| corrupt("orchestrator version", error))
  }
}

/// What the Orchestrator runs on, as a row holds it from column `first` on.
struct RawSettings {
  connection: String,
  model: String,
  effort: Option<String>,
}

fn settings_row(row: &Row<'_>, first: usize) -> rusqlite::Result<RawSettings> {
  Ok(RawSettings {
    connection: row.get(first)?,
    model: row.get(first + 1)?,
    effort: row.get(first + 2)?,
  })
}

impl RawSettings {
  fn into_settings(self) -> Result<OrchestratorSettings, StoreError> {
    let model = ModelId::new(&self.model).map_err(|error| corrupt("orchestrator model", error))?;
    Ok(OrchestratorSettings::new(
      parse("orchestrator connection", &self.connection)?,
      model,
      self.effort,
    ))
  }
}
