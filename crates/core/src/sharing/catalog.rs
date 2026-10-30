//! What this computer reports to a shared project's server for its web editor to offer (`docs/architecture/server.md`,
//! "References catalog", "Models per role", row 75): every agent connection and MCP server by kind, name and role, and
//! per connection role the model ids its connections list. [`catalog`] is the one function that builds it, from the
//! names, kinds and model lists alone, so no account, path, program, setting or key can reach a report.

use std::collections::BTreeMap;

use super::ConnectionRole;
use crate::{
  connection::{AgentConnection, Connection, mcp::McpServer},
  resource::ModelId,
};

/// The most references one report may carry.
pub const CATALOG_REFERENCES: usize = 256;
/// The most model ids one report may carry for one role.
pub const CATALOG_MODELS_PER_ROLE: usize = 512;

/// What a reported entry is. Secrets are never reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CatalogEntryKind {
  Connection,
  McpServer,
}

/// A connection or an MCP server as the report names it: its kind, the person's own label and, for a connection, its
/// role ([`ConnectionRole::as_str`]). An MCP server has no role.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CatalogEntry {
  pub kind: CatalogEntryKind,
  pub name: String,
  pub role: Option<&'static str>,
}

/// This computer's whole catalog for one project: its entries, connections first, each in the order given, and per
/// connection role the model ids its connections list, sorted and once each.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalCatalog {
  pub entries: Vec<CatalogEntry>,
  pub models: BTreeMap<&'static str, Vec<ModelId>>,
}

/// The catalog of `connections` and `mcp_servers`: the agent connections among `connections` and every MCP server, at
/// most [`CATALOG_REFERENCES`] of them, and at most [`CATALOG_MODELS_PER_ROLE`] model ids per role.
pub fn catalog(connections: &[Connection], mcp_servers: &[McpServer]) -> LocalCatalog {
  let agents: Vec<AgentConnection> = connections
    .iter()
    // Other kinds are not agent connections; the conversion error is how they are left out.
    .filter_map(|connection| AgentConnection::try_from(connection.clone()).ok())
    .collect();
  let connection_entries = agents.iter().map(|agent| CatalogEntry {
    kind: CatalogEntryKind::Connection,
    name: agent.name().as_str().to_owned(),
    role: Some(ConnectionRole::of(agent.kind()).as_str()),
  });
  let server_entries = mcp_servers.iter().map(|server| CatalogEntry {
    kind: CatalogEntryKind::McpServer,
    name: server.name().as_str().to_owned(),
    role: None,
  });
  let entries = connection_entries
    .chain(server_entries)
    .take(CATALOG_REFERENCES)
    .collect();
  let mut models: BTreeMap<&'static str, Vec<ModelId>> = BTreeMap::new();
  for agent in &agents {
    let role = models.entry(ConnectionRole::of(agent.kind()).as_str()).or_default();
    role.extend(agent.models().iter().map(|model| model.id().clone()));
  }
  for ids in models.values_mut() {
    ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    ids.dedup();
    ids.truncate(CATALOG_MODELS_PER_ROLE);
  }
  // A role whose connections list no model says nothing about models.
  models.retain(|_, ids| !ids.is_empty());
  LocalCatalog { entries, models }
}
