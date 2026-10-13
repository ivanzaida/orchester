//! Versioned operation records and retained originals. Updating metadata never overwrites recovery content.

mod process;

use orchester_core::{
  blob::ContentHash,
  id::RunId,
  integration::local::{
    LocalIntegrationStore, LocalOperation, LocalPath, LocalPhase, LocalRecoveryStore, RecoveryLayer,
  },
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl LocalIntegrationStore for ProjectDb {
  type Error = StoreError;

  fn local_operation(&self, run: RunId) -> Result<Option<LocalOperation>, StoreError> {
    let body: Option<(i64, String)> = self
      .connection
      .query_row(
        "SELECT version, body FROM local_integrations WHERE run_id = ?1",
        [run.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    body.map(|(version, body)| decode(run, version, &body)).transpose()
  }

  fn local_operations(&self) -> Result<Vec<LocalOperation>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT run_id, version, body FROM local_integrations ORDER BY run_id")?;
    let rows = statement.query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, String>(2)?,
      ))
    })?;
    rows
      .map(|row| {
        let (run, version, body) = row?;
        let run = run
          .parse()
          .map_err(|_| corrupt("local integration", "invalid row identity"))?;
        decode(run, version, &body)
      })
      .collect()
  }

  fn save_local_operation(&mut self, operation: &LocalOperation, expected: Option<u64>) -> Result<bool, StoreError> {
    let body = serde_json::to_string(operation).map_err(|error| corrupt("local integration", error))?;
    let version = i64::try_from(operation.version()).map_err(|_| StoreError::Unrepresentable("operation version"))?;
    let changed = match expected {
      None => self.connection.execute(
        "INSERT INTO local_integrations (run_id, version, body) VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING",
        params![operation.run().to_string(), version, body],
      )?,
      Some(previous) => {
        if previous.checked_add(1) != Some(operation.version()) {
          return Err(corrupt("local integration version", "updates advance by exactly one"));
        }
        if let Some(saved) = self.local_operation(operation.run())? {
          if saved.version() != previous {
            return Ok(false);
          }
          if saved.reviewed().root != operation.reviewed().root
            || saved.reviewed().git_dir != operation.reviewed().git_dir
            || saved
              .merged()
              .is_some_and(|revision| operation.merged() != Some(revision))
          {
            return Err(corrupt(
              "local integration",
              "checkout ownership and completed merge cannot be replaced",
            ));
          }
        }
        let previous = i64::try_from(previous).map_err(|_| StoreError::Unrepresentable("operation version"))?;
        self.connection.execute(
          "UPDATE local_integrations SET version = ?2, body = ?3 WHERE run_id = ?1 AND version = ?4",
          params![operation.run().to_string(), version, body, previous],
        )?
      }
    };
    Ok(changed == 1)
  }
}

impl LocalRecoveryStore for ProjectDb {
  fn retain_local_content(
    &mut self,
    run: RunId,
    path: &LocalPath,
    layer: RecoveryLayer,
    hash: &ContentHash,
    content: &[u8],
  ) -> Result<(), StoreError> {
    verify(hash.as_str(), content)?;
    self.connection.execute(
      "INSERT INTO local_recovery_content (run_id, path, layer, digest, content) VALUES (?1, ?2, ?3, ?4, ?5) \
       ON CONFLICT DO NOTHING",
      params![run.to_string(), path.as_str(), layer.as_str(), hash.as_str(), content],
    )?;
    let retained = self.local_content(run, path, layer)?;
    if retained.as_deref() != Some(content) {
      return Err(corrupt("local recovery", "retained originals cannot be replaced"));
    }
    Ok(())
  }

  fn local_content(&self, run: RunId, path: &LocalPath, layer: RecoveryLayer) -> Result<Option<Vec<u8>>, StoreError> {
    let row: Option<(String, Vec<u8>)> = self
      .connection
      .query_row(
        "SELECT digest, content FROM local_recovery_content WHERE run_id = ?1 AND path = ?2 AND layer = ?3",
        params![run.to_string(), path.as_str(), layer.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    row
      .map(|(digest, content)| {
        verify(&digest, &content)?;
        Ok(content)
      })
      .transpose()
  }

  fn remove_local_content(&mut self, run: RunId) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let body: String = transaction.query_row(
      "SELECT body FROM local_integrations WHERE run_id = ?1",
      [run.to_string()],
      |row| row.get(0),
    )?;
    let operation: LocalOperation = serde_json::from_str(&body).map_err(|error| corrupt("local integration", error))?;
    if operation.phase() != LocalPhase::RemovingPreservation {
      return Err(corrupt("local recovery", "restoration must be verified before removal"));
    }
    transaction.execute(
      "DELETE FROM local_recovery_content WHERE run_id = ?1",
      [run.to_string()],
    )?;
    transaction.commit()?;
    Ok(())
  }
}

fn verify(expected: &str, content: &[u8]) -> Result<(), StoreError> {
  if super::blobs::BlobHash::of(content).as_str() != expected {
    return Err(corrupt(
      "local recovery",
      "content does not match its recorded fingerprint",
    ));
  }
  Ok(())
}
fn decode(run: RunId, version: i64, body: &str) -> Result<LocalOperation, StoreError> {
  let operation: LocalOperation = serde_json::from_str(body).map_err(|error| corrupt("local integration", error))?;
  if operation.run() != run || i64::try_from(operation.version()).ok() != Some(version) {
    return Err(corrupt("local integration", "record identity differs from its row"));
  }
  Ok(operation)
}
