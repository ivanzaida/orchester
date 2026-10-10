//! Storing the project's MCP servers, as the Connections screen and the setup tools change them: a new server or a
//! server's new settings, turning one on or off, and removing one no agent profile chooses. A server's program is
//! stored as the fixed file it is (`tools::program`), as a command tool's is. Which profiles choose a server is one
//! rule for the screen, the tools and removal ([`server_uses`]). Every function blocks on SQLite; the UI calls them
//! off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::mcp::{McpServer, McpServerError, McpTransport, StdioServer},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, ModelId, ResourceKind},
  store::{McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{McpServerDraft, NAME_TAKEN};
use crate::{
  messages::{self, Message},
  tools::program::{ProgramProblem, fixed_program},
};

/// Why a server was not stored or removed.
#[derive(Debug, Error)]
pub enum McpSaveError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Server(#[from] McpServerError),
  /// The program cannot be stored as a fixed file outside the repository.
  #[error("the MCP server's program cannot be stored: {0:?}")]
  Program(ProgramProblem),
  /// Agent profiles still choose the server, by their roles: a draft or the latest published version names it. They
  /// switch first.
  #[error("agent profiles still choose the MCP server")]
  InUse(Vec<Arc<str>>),
}

impl McpSaveError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(StoreError::DuplicateConnectionName(_)) => Message::new(NAME_TAKEN),
      Self::Store(error) => {
        tracing::error!(%error, "cannot store the MCP server");
        messages::store_error(error)
      }
      Self::Server(McpServerError::EnvironmentOnHttp) => Message::new("connections.mcp.errors.variables"),
      Self::Program(problem) => program_problem_message(problem),
      Self::InUse(roles) => Message::new("connections.mcp.errors.inUse").with("profiles", roles.join(", ")),
    }
  }
}

/// Why `problem` keeps a program from being stored, in words.
pub fn program_problem_message(problem: &ProgramProblem) -> Message {
  match problem {
    ProgramProblem::InWorktree => Message::new("connections.mcp.errors.programInWorktree"),
    ProgramProblem::NotFound(named) => {
      Message::new("connections.mcp.errors.programNotFound").with("program", named.clone())
    }
    ProgramProblem::NotLocal(named) => {
      Message::new("connections.mcp.errors.programNotLocal").with("program", named.clone())
    }
    ProgramProblem::InRepository(found) => {
      Message::new("connections.mcp.errors.programInRepository").with("program", found.display().to_string())
    }
    ProgramProblem::NotText(found) => {
      Message::new("connections.mcp.errors.programNotText").with("program", found.display().to_string())
    }
    ProgramProblem::Invalid(error) => crate::tools::form::program_message(error),
  }
}

/// Stores `draft` as a new server, or as server `id`'s new settings, keeping whether it is on. A program another
/// than the stored one is stored as the fixed file it is, looked up on this app's search path, outside `repo_root`.
pub fn store_in(
  repo_root: &Path,
  id: Option<ConnectionId>,
  draft: McpServerDraft,
) -> Result<ConnectionId, McpSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let stored = match id {
    Some(id) => Some(db.mcp_server(id)?.ok_or(StoreError::ConnectionNotFound(id))?),
    None => None,
  };
  let transport = fixed_transport(repo_root, stored.as_ref(), draft.transport)?;
  let Some(mut server) = stored else {
    let mut server = McpServer::new(
      ConnectionId::generate(),
      draft.name,
      transport,
      draft.environment,
      Timestamp::now(),
    )?;
    server.set_exempt_tools(draft.exempt_tools);
    db.create_mcp_server(&server)?;
    return Ok(server.id());
  };
  server.update(draft.name, transport, draft.environment)?;
  server.set_exempt_tools(draft.exempt_tools);
  db.save_mcp_server(&server)?;
  Ok(server.id())
}

/// `transport` with its program fixed, unless it is the program `stored` already has.
fn fixed_transport(
  repo_root: &Path,
  stored: Option<&McpServer>,
  transport: McpTransport,
) -> Result<McpTransport, McpSaveError> {
  let McpTransport::Stdio(stdio) = transport else {
    return Ok(transport);
  };
  let unchanged = matches!(stored.map(McpServer::transport), Some(McpTransport::Stdio(before)) if before.program() == stdio.program());
  if unchanged {
    return Ok(McpTransport::Stdio(stdio));
  }
  let path = orchester_exec::search_path::search_path();
  let program = fixed_program(stdio.program(), path.as_deref(), repo_root).map_err(McpSaveError::Program)?;
  let directory = stdio.working_directory().cloned();
  Ok(McpTransport::Stdio(StdioServer::new(
    program,
    stdio.args().to_vec(),
    directory,
  )))
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

/// Removes server `id` once no agent profile chooses it ([`server_uses`]), checked and removed in one transaction. A
/// profile that does would lose the server without a person deciding which profile works without it, so the
/// profiles switch first.
pub fn remove_in(repo_root: &Path, id: ConnectionId) -> Result<(), McpSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let users = db.delete_unused_mcp_server(id, &move |db| profiles_using(db, id))?;
  if users.is_empty() {
    Ok(())
  } else {
    Err(McpSaveError::InUse(users))
  }
}

/// A profile that gives its agents an MCP server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerUse {
  pub server: ConnectionId,
  pub profile: ResourceId,
  pub role: Arc<str>,
  /// The model of the version that names the server: the draft's when the draft names it.
  pub model: ModelId,
}

/// Every profile's servers: those its draft names and those its latest published version names, in profile
/// order, each once. A run may start from the published version while the draft is edited, so both count.
pub fn server_uses<S: ResourceStore>(store: &S) -> Result<Vec<ServerUse>, S::Error> {
  let mut uses: Vec<ServerUse> = Vec::new();
  for resource in store.resources(ResourceKind::AgentProfile)? {
    let draft = store
      .draft::<AgentProfile>(resource.id())?
      .map(|draft| draft.body().clone());
    let latest = store
      .latest_version::<AgentProfile>(resource.id())?
      .map(|version| version.body().clone());
    for body in draft.iter().chain(latest.iter()) {
      for server in body.mcp_servers().ids() {
        if !uses
          .iter()
          .any(|used| used.server == *server && used.profile == resource.id())
        {
          uses.push(ServerUse {
            server: *server,
            profile: resource.id(),
            role: Arc::from(body.role().as_str()),
            model: body.model().clone(),
          });
        }
      }
    }
  }
  Ok(uses)
}

/// The roles of the profiles that choose server `id` ([`server_uses`]).
pub fn profiles_using<S: ResourceStore>(store: &S, id: ConnectionId) -> Result<Vec<Arc<str>>, S::Error> {
  Ok(
    server_uses(store)?
      .into_iter()
      .filter(|used| used.server == id)
      .map(|used| used.role)
      .collect(),
  )
}
