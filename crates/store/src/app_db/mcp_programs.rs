//! The program of a team MCP server on this computer (`docs/design/team-configuration.md`, decision D of 2026-11-06):
//! the person sets the path once and every project that uses that team MCP server shares it. It is kept per team server
//! (as registered on this computer) and team MCP server resource, here and not in a project database. A path whose
//! file is gone stays stored; whoever shows it marks it.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::{ResourceId, ServerId},
  time::Timestamp,
};
use rusqlite::params;

use super::AppDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

pub(super) const MIGRATION: &str = r"
CREATE TABLE mcp_server_programs (
  server TEXT NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
  resource TEXT NOT NULL,
  program TEXT NOT NULL,
  set_at INTEGER NOT NULL,
  PRIMARY KEY (server, resource)
) STRICT;
";

/// The program path set for one team MCP server on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpServerProgram {
  pub resource: ResourceId,
  pub program: PathBuf,
  pub set_at: Timestamp,
}

impl AppDb {
  /// Keeps `program` as the program of the team MCP server `resource` of `server`, in place of an earlier one.
  /// Refused for a server this computer does not know and for a path that is not Unicode, which the database cannot
  /// keep exactly.
  pub fn set_mcp_server_program(
    &mut self,
    server: ServerId,
    resource: ResourceId,
    program: &Path,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let program = program
      .to_str()
      .ok_or(StoreError::UnrepresentablePath("MCP server program"))?;
    let changed = self.connection.execute(
      "INSERT INTO mcp_server_programs (server, resource, program, set_at)
       SELECT id, ?2, ?3, ?4 FROM servers WHERE id = ?1
       ON CONFLICT (server, resource) DO UPDATE SET program = excluded.program, set_at = excluded.set_at",
      params![server.to_string(), resource.to_string(), program, at.unix_millis()],
    )?;
    if changed == 0 {
      return Err(StoreError::ServerNotFound(server));
    }
    Ok(())
  }

  /// Forgets the program of the team MCP server `resource` of `server`; whether there was one.
  pub fn clear_mcp_server_program(&mut self, server: ServerId, resource: ResourceId) -> Result<bool, StoreError> {
    let removed = self.connection.execute(
      "DELETE FROM mcp_server_programs WHERE server = ?1 AND resource = ?2",
      params![server.to_string(), resource.to_string()],
    )?;
    Ok(removed > 0)
  }

  /// The program set for the team MCP server `resource` of `server`, if any. The file may be gone.
  pub fn mcp_server_program(
    &self,
    server: ServerId,
    resource: ResourceId,
  ) -> Result<Option<McpServerProgram>, StoreError> {
    Ok(
      self
        .mcp_server_programs(server)?
        .into_iter()
        .find(|program| program.resource == resource),
    )
  }

  /// Every program set for the team MCP servers of `server`, by resource.
  pub fn mcp_server_programs(&self, server: ServerId) -> Result<Vec<McpServerProgram>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT resource, program, set_at FROM mcp_server_programs WHERE server = ?1 ORDER BY resource")?;
    let rows = statement
      .query_map(params![server.to_string()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, i64>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(resource, program, set_at)| {
        if program.is_empty() {
          return Err(corrupt("MCP server program", "empty path"));
        }
        Ok(McpServerProgram {
          resource: parse("MCP server program resource", &resource)?,
          program: PathBuf::from(program),
          set_at: Timestamp::from_unix_millis(set_at),
        })
      })
      .collect()
  }
}
