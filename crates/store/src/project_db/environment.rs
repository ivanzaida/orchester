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
  transaction.execute("DELETE FROM project_environment", [])?;
  {
    let mut insert = transaction.prepare("INSERT INTO project_environment (name, value) VALUES (?1, ?2)")?;
    for (name, value) in environment.iter() {
      insert.execute(params![name.as_str(), value])?;
    }
  }
  transaction.commit()?;
  Ok(())
}
