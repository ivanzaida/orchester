//! The `mcp_servers` table. A variable whose value comes from a secret stores the secret's id; the
//! value itself is only in the OS keychain. A row that cannot be read back is left out of the list
//! and reported on its own ([`UnreadableMcpServer`]), so one damaged row never hides the others. The
//! tools that run without asking are part of the settings since they were added; settings written
//! before then have none.

use orchester_core::{
  connection::mcp::{ExemptTools, McpEnvironment, McpServer, McpServerName, McpTransport, UnreadableMcpServer},
  id::ConnectionId,
  store::McpServerStore,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Params, Row, params};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const SELECT: &str = "SELECT id, name, settings, enabled, created_at FROM mcp_servers";
const WHAT: &str = "MCP server settings";
const SETTINGS_SCHEMA: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Settings {
  transport: McpTransport,
  environment: McpEnvironment,
  #[serde(default, skip_serializing_if = "ExemptTools::is_empty")]
  exempt_tools: ExemptTools,
}

impl McpServerStore for ProjectDb {
  type Error = StoreError;

  fn create_mcp_server(&mut self, server: &McpServer) -> Result<(), StoreError> {
    let settings = settings(server)?;
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, server)?;
    transaction.execute(
      "INSERT INTO mcp_servers (id, name, settings, enabled, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        server.id().to_string(),
        server.name().as_str(),
        settings,
        server.enabled(),
        server.created_at().unix_millis()
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  fn save_mcp_server(&mut self, server: &McpServer) -> Result<(), StoreError> {
    let settings = settings(server)?;
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, server)?;
    let changed = transaction.execute(
      "UPDATE mcp_servers SET name = ?2, settings = ?3, enabled = ?4 WHERE id = ?1",
      params![
        server.id().to_string(),
        server.name().as_str(),
        settings,
        server.enabled()
      ],
    )?;
    if changed == 0 {
      return Err(StoreError::ConnectionNotFound(server.id()));
    }
    transaction.commit()?;
    Ok(())
  }

  fn mcp_server(&self, id: ConnectionId) -> Result<Option<McpServer>, StoreError> {
    stored(&self.connection, "WHERE id = ?1", [id.to_string()])?
      .pop()
      .map(StoredRow::into_server)
      .transpose()
  }

  fn mcp_servers(&self) -> Result<Vec<McpServer>, StoreError> {
    let rows = stored(&self.connection, "ORDER BY name", [])?;
    // A row that cannot be read back is left out, not lost: `unreadable_mcp_servers` reports it.
    Ok(rows.into_iter().filter_map(|row| row.into_server().ok()).collect())
  }

  fn unreadable_mcp_servers(&self) -> Result<Vec<UnreadableMcpServer>, StoreError> {
    let rows = stored(&self.connection, "ORDER BY name", [])?;
    Ok(
      rows
        .into_iter()
        .filter_map(|row| {
          let (id, name) = (row.id.clone(), row.name.clone());
          let error = row.into_server().err()?;
          Some(UnreadableMcpServer {
            id,
            name,
            detail: error.to_string(),
          })
        })
        .collect(),
    )
  }
}

fn settings(server: &McpServer) -> Result<String, StoreError> {
  let settings = Settings {
    transport: server.transport().clone(),
    environment: server.environment().clone(),
    exempt_tools: server.exempt_tools().clone(),
  };
  encode_json(WHAT, SETTINGS_SCHEMA, &settings)
}

/// Fails if another MCP server already has this server's name.
fn ensure_name_free(sqlite: &Connection, server: &McpServer) -> Result<(), StoreError> {
  let taken: Option<String> = sqlite
    .query_row(
      "SELECT id FROM mcp_servers WHERE name = ?1 AND id <> ?2",
      params![server.name().as_str(), server.id().to_string()],
      |row| row.get(0),
    )
    .optional()?;
  match taken {
    Some(_) => Err(StoreError::DuplicateConnectionName(server.name().as_str().to_owned())),
    None => Ok(()),
  }
}

/// The stored rows `clause` selects, not yet checked against the rules of a server.
fn stored(sqlite: &Connection, clause: &str, params: impl Params) -> Result<Vec<StoredRow>, StoreError> {
  let mut statement = sqlite.prepare(&format!("{SELECT} {clause}"))?;
  let rows = statement
    .query_map(params, StoredRow::read)?
    .collect::<Result<Vec<_>, _>>()?;
  Ok(rows)
}

struct StoredRow {
  id: String,
  name: String,
  settings: String,
  enabled: bool,
  created_at: i64,
}

impl StoredRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      name: row.get(1)?,
      settings: row.get(2)?,
      enabled: row.get(3)?,
      created_at: row.get(4)?,
    })
  }

  fn into_server(self) -> Result<McpServer, StoreError> {
    let settings: Settings = decode_json(WHAT, SETTINGS_SCHEMA, &self.settings)?;
    let mut server = McpServer::from_stored(
      parse("MCP server id", &self.id)?,
      McpServerName::new(&self.name).map_err(|error| corrupt("MCP server name", error))?,
      (settings.transport, settings.environment),
      self.enabled,
      Timestamp::from_unix_millis(self.created_at),
    )
    .map_err(|error| corrupt("MCP server", error))?;
    server.set_exempt_tools(settings.exempt_tools);
    Ok(server)
  }
}
