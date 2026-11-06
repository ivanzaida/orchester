//! Decision model servers, with secret references only and versioned check/test records.

use orchester_core::{
  connection::{DecisionCheck, DecisionConnection, DecisionServerUrl, DecisionTest},
  id::{DecisionConnectionId, SecretId},
  name::Name,
  resource::ModelId,
};
use rusqlite::{Connection, OptionalExtension, Params, params};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const WHAT: &str = "decision connection settings";
const SCHEMA: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Settings {
  base_url: DecisionServerUrl,
  key: Option<SecretId>,
  models: Vec<ModelId>,
  last_check: Option<DecisionCheck>,
  last_test: Option<DecisionTest>,
}

impl Settings {
  fn of(connection: &DecisionConnection) -> Self {
    Self {
      base_url: connection.base_url().clone(),
      key: connection.key(),
      models: connection.models().to_vec(),
      last_check: connection.last_check().cloned(),
      last_test: connection.last_test().cloned(),
    }
  }

  fn into_connection(self, id: DecisionConnectionId, name: Name) -> DecisionConnection {
    let mut connection = DecisionConnection::new(id, name, self.base_url, self.key).with_models(self.models);
    if let Some(check) = self.last_check {
      connection.record_check(check);
    }
    if let Some(test) = self.last_test {
      connection.record_test(test);
    }
    connection
  }
}

impl ProjectDb {
  pub fn create_decision_connection(&mut self, connection: &DecisionConnection) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, connection)?;
    transaction.execute(
      "INSERT INTO decision_connections (id, name, settings) VALUES (?1, ?2, ?3)",
      params![
        connection.id().to_string(),
        connection.name().as_str(),
        encode_json(WHAT, SCHEMA, &Settings::of(connection))?
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  pub fn save_decision_connection(&mut self, connection: &DecisionConnection) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, connection)?;
    let changed = transaction.execute(
      "UPDATE decision_connections SET name = ?2, settings = ?3 WHERE id = ?1",
      params![
        connection.id().to_string(),
        connection.name().as_str(),
        encode_json(WHAT, SCHEMA, &Settings::of(connection))?
      ],
    )?;
    if changed == 0 {
      return Err(StoreError::DecisionConnectionNotFound(connection.id()));
    }
    transaction.commit()?;
    Ok(())
  }

  /// Saves check settings only if the entire loaded connection still matches, preserving concurrent edits.
  pub fn save_decision_check_if_unchanged(
    &mut self,
    expected: &DecisionConnection,
    checked: &DecisionConnection,
  ) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE decision_connections SET settings = ?1 WHERE id = ?2 AND name = ?3 AND settings = ?4",
      params![
        encode_json(WHAT, SCHEMA, &Settings::of(checked))?,
        expected.id().to_string(),
        expected.name().as_str(),
        encode_json(WHAT, SCHEMA, &Settings::of(expected))?
      ],
    )?;
    Ok(changed != 0)
  }

  /// Saves an edit only while the displayed version still matches, under the same write lock.
  pub fn save_decision_connection_if_unchanged(
    &mut self,
    expected: &DecisionConnection,
    edited: &DecisionConnection,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, edited)?;
    let changed = transaction.execute(
      "UPDATE decision_connections SET name = ?1, settings = ?2 WHERE id = ?3 AND name = ?4 AND settings = ?5",
      params![
        edited.name().as_str(),
        encode_json(WHAT, SCHEMA, &Settings::of(edited))?,
        expected.id().to_string(),
        expected.name().as_str(),
        encode_json(WHAT, SCHEMA, &Settings::of(expected))?
      ],
    )?;
    transaction.commit()?;
    Ok(changed != 0)
  }

  pub fn decision_connection(&self, id: DecisionConnectionId) -> Result<Option<DecisionConnection>, StoreError> {
    Ok(load(&self.connection, "WHERE id = ?1", [id.to_string()])?.pop())
  }

  pub fn decision_connections(&self) -> Result<Vec<DecisionConnection>, StoreError> {
    load(&self.connection, "ORDER BY name", [])
  }

  /// Refuses removal while the project's reviewer selects this connection, under the same write lock.
  pub fn delete_decision_connection(&mut self, id: DecisionConnectionId) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if reviewer_connection(&transaction)? == Some(id) {
      return Err(StoreError::DecisionConnectionInUse(id));
    }
    if transaction.execute("DELETE FROM decision_connections WHERE id = ?1", [id.to_string()])? == 0 {
      return Err(StoreError::DecisionConnectionNotFound(id));
    }
    transaction.commit()?;
    Ok(())
  }

  pub fn decision_reviewer_connection(&self) -> Result<Option<DecisionConnectionId>, StoreError> {
    reviewer_connection(&self.connection)
  }

  /// Persists the reviewer's connection reference; foreign keys also prevent dangling selections.
  pub fn save_decision_reviewer_connection(&mut self, id: Option<DecisionConnectionId>) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if let Some(id) = id {
      if load(&transaction, "WHERE id = ?1", [id.to_string()])?.is_empty() {
        return Err(StoreError::DecisionConnectionNotFound(id));
      }
      transaction.execute(
        "INSERT INTO decision_reviewer_connection (id, connection_id) VALUES (1, ?1) \
         ON CONFLICT(id) DO UPDATE SET connection_id = excluded.connection_id, \
         settings = CASE WHEN connection_id = excluded.connection_id THEN settings ELSE NULL END, \
         model = CASE WHEN connection_id = excluded.connection_id THEN model ELSE NULL END",
        [id.to_string()],
      )?;
    } else {
      transaction.execute("DELETE FROM decision_reviewer_connection", [])?;
    }
    transaction.commit()?;
    Ok(())
  }
}

fn reviewer_connection(connection: &Connection) -> Result<Option<DecisionConnectionId>, StoreError> {
  let id: Option<String> = connection
    .query_row(
      "SELECT connection_id FROM decision_reviewer_connection WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .optional()?;
  id.map(|id| parse("decision reviewer connection", &id)).transpose()
}

fn ensure_name_free(connection: &Connection, value: &DecisionConnection) -> Result<(), StoreError> {
  let taken: bool = connection.query_row(
    "SELECT EXISTS(SELECT 1 FROM decision_connections WHERE name = ?1 AND id <> ?2)",
    params![value.name().as_str(), value.id().to_string()],
    |row| row.get(0),
  )?;
  if taken {
    return Err(StoreError::DuplicateConnectionName(value.name().as_str().to_owned()));
  }
  Ok(())
}

pub(super) fn load(
  connection: &Connection,
  clause: &str,
  params: impl Params,
) -> Result<Vec<DecisionConnection>, StoreError> {
  let mut statement = connection.prepare(&format!("SELECT id, name, settings FROM decision_connections {clause}"))?;
  let rows = statement
    .query_map(params, |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(id, name, settings)| {
      let settings: Settings = decode_json(WHAT, SCHEMA, &settings)?;
      Ok(settings.into_connection(
        parse("decision connection id", &id)?,
        Name::new(&name).map_err(|error| corrupt("decision connection name", error))?,
      ))
    })
    .collect()
}
