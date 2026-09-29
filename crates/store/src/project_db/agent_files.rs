//! `project_agent_files`: Project settings › General › Repository agent files, one row; a project
//! that never changed it has it on.

use orchester_core::agent_files::RepositoryAgentFiles;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{StoreError, rows::corrupt};

pub(super) fn load(connection: &Connection) -> Result<RepositoryAgentFiles, StoreError> {
  let setting: Option<String> = connection
    .query_row("SELECT setting FROM project_agent_files", [], |row| row.get(0))
    .optional()?;
  match setting.as_deref() {
    None | Some("on") => Ok(RepositoryAgentFiles::On),
    Some("off") => Ok(RepositoryAgentFiles::Off),
    Some(other) => Err(corrupt("repository agent files setting", other)),
  }
}

pub(super) fn save(connection: &Connection, setting: RepositoryAgentFiles) -> Result<(), StoreError> {
  let text = match setting {
    RepositoryAgentFiles::On => "on",
    RepositoryAgentFiles::Off => "off",
  };
  connection.execute(
    "INSERT INTO project_agent_files (id, setting) VALUES (1, ?1) \
     ON CONFLICT (id) DO UPDATE SET setting = excluded.setting",
    params![text],
  )?;
  Ok(())
}
