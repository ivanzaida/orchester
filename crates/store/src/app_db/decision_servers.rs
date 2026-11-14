//! The person's decision model servers (`docs/design/agent-accounts.md`, "Agents run on accounts", decision 3): each
//! one's name, base URL, the models its last check listed and that check, kept once for every project. A server that
//! takes a key names its own id as the key's; the key itself is never here: it is in the OS keychain under that id.
//! A project that reviews with one has a copy of it among its decision connections, by the same id.

use orchester_core::{connection::DecisionConnection, id::DecisionConnectionId, name::Name, time::Timestamp};
use rusqlite::{Connection, Params, params};

use super::AppDb;
use crate::{
  StoreError, decision_settings,
  rows::{corrupt, parse},
};

pub(super) const MIGRATION: &str = r"
CREATE TABLE decision_servers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  settings TEXT NOT NULL,
  added_at INTEGER NOT NULL
) STRICT;

CREATE UNIQUE INDEX decision_server_names ON decision_servers (lower(name));
";

impl AppDb {
  /// The decision model servers, in the order they were added.
  pub fn decision_servers(&self) -> Result<Vec<DecisionConnection>, StoreError> {
    load(&self.connection, "ORDER BY added_at, id", [])
  }

  pub fn decision_server(&self, id: DecisionConnectionId) -> Result<Option<DecisionConnection>, StoreError> {
    Ok(load(&self.connection, "WHERE id = ?1", [id.to_string()])?.pop())
  }

  /// Adds `server`; a name another server has, whatever its case, is refused.
  pub fn add_decision_server(&mut self, server: &DecisionConnection, now: Timestamp) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, server)?;
    transaction.execute(
      "INSERT INTO decision_servers (id, name, settings, added_at) VALUES (?1, ?2, ?3, ?4)",
      params![
        server.id().to_string(),
        server.name().as_str(),
        decision_settings::encode(server)?,
        now.unix_millis()
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// Keeps `server` as it is now: its name, server, models, check and test.
  pub fn save_decision_server(&mut self, server: &DecisionConnection) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, server)?;
    let changed = transaction.execute(
      "UPDATE decision_servers SET name = ?2, settings = ?3 WHERE id = ?1",
      params![
        server.id().to_string(),
        server.name().as_str(),
        decision_settings::encode(server)?
      ],
    )?;
    if changed == 0 {
      return Err(StoreError::DecisionConnectionNotFound(server.id()));
    }
    transaction.commit()?;
    Ok(())
  }

  /// Forgets the server; whether there was one.
  pub fn remove_decision_server(&mut self, id: DecisionConnectionId) -> Result<bool, StoreError> {
    let removed = self
      .connection
      .execute("DELETE FROM decision_servers WHERE id = ?1", [id.to_string()])?;
    Ok(removed != 0)
  }
}

fn ensure_name_free(connection: &Connection, server: &DecisionConnection) -> Result<(), StoreError> {
  let taken: bool = connection.query_row(
    "SELECT EXISTS(SELECT 1 FROM decision_servers WHERE lower(name) = lower(?1) AND id <> ?2)",
    params![server.name().as_str(), server.id().to_string()],
    |row| row.get(0),
  )?;
  if taken {
    return Err(StoreError::DuplicateConnectionName(server.name().as_str().to_owned()));
  }
  Ok(())
}

fn load(connection: &Connection, clause: &str, params: impl Params) -> Result<Vec<DecisionConnection>, StoreError> {
  let mut statement = connection.prepare(&format!("SELECT id, name, settings FROM decision_servers {clause}"))?;
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
      decision_settings::decode(
        parse("decision server id", &id)?,
        Name::new(&name).map_err(|error| corrupt("decision server name", error))?,
        &settings,
      )
    })
    .collect()
}
