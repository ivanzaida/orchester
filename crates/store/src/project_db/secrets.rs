//! The `secrets` table: a secret's name, kind, environment and when it was added, rotated and
//! archived. No column holds its value; only the OS keychain does.

use orchester_core::{
  id::SecretId,
  secret::{NewSecret, Secret, SecretName},
  store::SecretStore,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Params, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

const SELECT: &str = "SELECT id, name, kind, environment, created_at, rotated_at, archived_at FROM secrets";

impl SecretStore for ProjectDb {
  type Error = StoreError;

  fn create_secret(&mut self, secret: &Secret) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, secret)?;
    transaction.execute(
      "INSERT INTO secrets (id, name, kind, environment, created_at, rotated_at, archived_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        secret.id().to_string(),
        secret.name().as_str(),
        secret.kind().as_str(),
        secret.environment().as_str(),
        secret.created_at().unix_millis(),
        secret.rotated_at().map(Timestamp::unix_millis),
        secret.archived_at().map(Timestamp::unix_millis)
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  fn save_secret(&mut self, secret: &Secret) -> Result<(), StoreError> {
    let changed = self.connection.execute(
      "UPDATE secrets SET rotated_at = ?2, archived_at = ?3 WHERE id = ?1",
      params![
        secret.id().to_string(),
        secret.rotated_at().map(Timestamp::unix_millis),
        secret.archived_at().map(Timestamp::unix_millis)
      ],
    )?;
    if changed == 0 {
      return Err(StoreError::SecretNotFound(secret.id()));
    }
    Ok(())
  }

  fn secret(&self, id: SecretId) -> Result<Option<Secret>, StoreError> {
    Ok(load(&self.connection, "WHERE id = ?1", [id.to_string()])?.pop())
  }

  fn secrets(&self) -> Result<Vec<Secret>, StoreError> {
    load(
      &self.connection,
      "WHERE archived_at IS NULL ORDER BY name, environment",
      [],
    )
  }
}

/// Fails if another active secret has this secret's name in its environment.
fn ensure_name_free(sqlite: &Connection, secret: &Secret) -> Result<(), StoreError> {
  let taken: Option<String> = sqlite
    .query_row(
      "SELECT id FROM secrets WHERE name = ?1 AND environment = ?2 AND archived_at IS NULL AND id <> ?3",
      params![
        secret.name().as_str(),
        secret.environment().as_str(),
        secret.id().to_string()
      ],
      |row| row.get(0),
    )
    .optional()?;
  match taken {
    Some(_) => Err(StoreError::DuplicateSecretName {
      name: secret.name().as_str().to_owned(),
      environment: secret.environment().as_str(),
    }),
    None => Ok(()),
  }
}

fn load(sqlite: &Connection, clause: &str, params: impl Params) -> Result<Vec<Secret>, StoreError> {
  let mut statement = sqlite.prepare(&format!("{SELECT} {clause}"))?;
  let rows = statement
    .query_map(params, StoredRow::read)?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(StoredRow::into_secret).collect()
}

struct StoredRow {
  id: String,
  name: String,
  kind: String,
  environment: String,
  created_at: i64,
  rotated_at: Option<i64>,
  archived_at: Option<i64>,
}

impl StoredRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      name: row.get(1)?,
      kind: row.get(2)?,
      environment: row.get(3)?,
      created_at: row.get(4)?,
      rotated_at: row.get(5)?,
      archived_at: row.get(6)?,
    })
  }

  fn into_secret(self) -> Result<Secret, StoreError> {
    let new = NewSecret {
      name: SecretName::new(&self.name).map_err(|error| corrupt("secret name", error))?,
      kind: parse("secret kind", &self.kind)?,
      environment: parse("secret environment", &self.environment)?,
    };
    Ok(Secret::from_stored(
      parse("secret id", &self.id)?,
      new,
      Timestamp::from_unix_millis(self.created_at),
      self.rotated_at.map(Timestamp::from_unix_millis),
      self.archived_at.map(Timestamp::from_unix_millis),
    ))
  }
}
