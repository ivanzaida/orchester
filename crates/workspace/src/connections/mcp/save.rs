//! Storing the project's MCP servers, as the Connections screen and the setup tools change them: a new
//! server or a server's new settings, turning one on or off, and removing one no agent profile chooses.
//! Every function blocks on SQLite; the UI calls them off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::mcp::{McpServer, McpServerError},
  id::ConnectionId,
  resource::{AgentProfile, ResourceKind},
  store::{McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::McpServerDraft;
use crate::messages::{self, Message};

/// Why a server was not stored or removed.
#[derive(Debug, Error)]
pub enum McpSaveError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Server(#[from] McpServerError),
  /// Agent profiles still choose the server, by their roles: a draft or the latest published version
  /// names it. They switch first.
  #[error("agent profiles still choose the MCP server")]
  InUse(Vec<Arc<str>>),
}

impl McpSaveError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(StoreError::DuplicateConnectionName(_)) => Message::new(super::NAME_TAKEN),
      Self::Store(error) => {
        tracing::error!(%error, "cannot store the MCP server");
        messages::store_error(error)
      }
      Self::Server(McpServerError::EnvironmentOnHttp) => Message::new("connections.mcp.errors.variables"),
      Self::InUse(roles) => Message::new("connections.mcp.errors.inUse").with("profiles", roles.join(", ")),
    }
  }
}

/// Stores `draft` as a new server, or as server `id`'s new settings, keeping whether it is on.
pub fn store_in(
  repo_root: &Path,
  id: Option<ConnectionId>,
  draft: McpServerDraft,
) -> Result<ConnectionId, McpSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let Some(id) = id else {
    let mut server = McpServer::new(
      ConnectionId::generate(),
      draft.name,
      draft.transport,
      draft.environment,
      Timestamp::now(),
    )?;
    server.set_exempt_tools(draft.exempt_tools);
    db.create_mcp_server(&server)?;
    return Ok(server.id());
  };
  let mut server = db.mcp_server(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  server.update(draft.name, draft.transport, draft.environment)?;
  server.set_exempt_tools(draft.exempt_tools);
  db.save_mcp_server(&server)?;
  Ok(id)
}

/// [`store_in`], its failure in words.
pub fn save_in(repo_root: &Path, id: Option<ConnectionId>, draft: McpServerDraft) -> Result<ConnectionId, Message> {
  store_in(repo_root, id, draft).map_err(|error| error.message())
}

/// Turns server `id` on or off for every profile that chooses it.
pub fn set_enabled_in(repo_root: &Path, id: ConnectionId, enabled: bool) -> Result<(), StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  let mut server = db.mcp_server(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  server.set_enabled(enabled);
  db.save_mcp_server(&server)
}

/// Removes server `id` once no agent profile chooses it: neither a draft nor a latest published version
/// names it. A profile that does would lose the server without a person deciding which profile works
/// without it, so the profiles switch first.
pub fn remove_in(repo_root: &Path, id: ConnectionId) -> Result<(), McpSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let users = profiles_using(&db, id)?;
  if !users.is_empty() {
    return Err(McpSaveError::InUse(users));
  }
  db.delete_mcp_server(id)?;
  Ok(())
}

/// The roles of the agent profiles whose draft or latest published version gives their agents server
/// `id`, in profile order.
pub fn profiles_using<S>(store: &S, id: ConnectionId) -> Result<Vec<Arc<str>>, S::Error>
where
  S: ResourceStore,
{
  let mut roles = Vec::new();
  for resource in store.resources(ResourceKind::AgentProfile)? {
    let draft = store.draft::<AgentProfile>(resource.id())?;
    let latest = store.latest_version::<AgentProfile>(resource.id())?;
    let named = |profile: &AgentProfile| profile.mcp_servers().contains(id);
    let role = match (&draft, &latest) {
      (Some(draft), _) if named(draft.body()) => Some(draft.body().role()),
      (_, Some(latest)) if named(latest.body()) => Some(latest.body().role()),
      _ => None,
    };
    if let Some(role) = role {
      roles.push(Arc::from(role.as_str()));
    }
  }
  Ok(roles)
}
