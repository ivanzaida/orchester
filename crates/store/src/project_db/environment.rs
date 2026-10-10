//! `project_environment`: the variables Project settings › Environment adds to every agent CLI
//! and command tool the project's runs start, one row per variable.

use std::collections::BTreeMap;

use orchester_core::resource::{CommandEnvironment, VariableName};
use rusqlite::{Connection, params};

use crate::{StoreError, rows::corrupt};

pub(super) fn load(connection: &Connection) -> Result<CommandEnvironment, StoreError> {
  let mut statement = connection.prepare("SELECT name, value FROM project_environment ORDER BY name")?;
  let rows = statement
    .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
    .collect::<Result<Vec<_>, _>>()?;
  let mut variables = BTreeMap::new();
  for (name, value) in rows {
    let name = VariableName::new(&name).map_err(|error| corrupt("environment variable", error))?;
    variables.insert(name, value);
  }
  CommandEnvironment::new(variables).map_err(|error| corrupt("project environment", error))
}

/// Replaces every stored variable with those of `environment`, atomically.
pub(super) fn save(connection: &mut Connection, environment: &CommandEnvironment) -> Result<(), StoreError> {
  let transaction = connection.transaction()?;
  write(&transaction, environment)?;
  transaction.commit()?;
  Ok(())
}

fn write(connection: &Connection, environment: &CommandEnvironment) -> Result<(), StoreError> {
  connection.execute("DELETE FROM project_environment", [])?;
  let mut insert = connection.prepare("INSERT INTO project_environment (name, value) VALUES (?1, ?2)")?;
  for (name, value) in environment.iter() {
    insert.execute(params![name.as_str(), value])?;
  }
  Ok(())
}

impl super::ProjectDb {
  /// Compare and save in one immediate transaction, so a confirmed environment cannot silently overwrite a concurrent
  /// change.
  pub fn save_environment_if_current(
    &mut self,
    base: &CommandEnvironment,
    asked: &CommandEnvironment,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    if load(&transaction)? != *base {
      return Ok(false);
    }
    write(&transaction, asked)?;
    transaction.commit()?;
    Ok(true)
  }
}
