//! `add_mcp_server`, `update_mcp_server`, `remove_mcp_server` and `set_mcp_server_enabled`: the project's MCP
//! servers as Connections › MCP servers adds, edits, turns on or off and removes them, through the same
//! fields and rules (`connections::mcp`). What widens what agents may do (`connections::mcp::widenings`: a new
//! server, a new name, another program, arguments, folder or endpoint, another authorization that sends a secret, a
//! variable but the harmless ones, a variable that takes a secret, a tool that runs without asking, turning a server
//! on) is refused as `person_only` unless a person lets automation change agent permissions, judged by the fields'
//! text before anything reads the disk; removing, turning off and dropping exempt tools are always taken. A program
//! is stored as the fixed file it is, outside the repository, as a command tool's (`tools::program`). A secret is
//! named, never its value, and only the project's active secrets can be named. In a team project an MCP server may be
//! the team's (`team_fill_ins`): turning it on and giving it a program are a person's alone, always.

use orchester_core::{
  connection::mcp::{McpServer, McpServerName},
  resource::ResourceKind,
  secret::Secret,
  store::{McpServerStore, SecretStore},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

pub use super::mcp_server_input::{AddMcpServer, InServer, SetMcpServerEnabled, UpdateMcpServer};
use super::{
  mcp_server_json::server_result,
  team_fill_ins::{self, team_resource_named},
  team_mcp_servers, widening,
};
pub(super) use super::{
  mcp_server_json::{servers_json, without_query_values},
  mcp_server_text::filled,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  connections::mcp::{self, Allowed, McpSaveError, McpServerText, NAME_TAKEN, enabling, widenings},
  messages::Message,
  tasks::outside::Outside,
  tools::program::ProgramProblem,
};

pub async fn add_mcp_server(services: &Services, args: AddMcpServer) -> Result<Value, ToolError> {
  if team_mcp_servers::is_team(services, &args.project).await? {
    return team_mcp_servers::add(services, args).await;
  }
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let secrets = db.secrets()?;
    let text = filled(McpServerText::new_server(), args.fields, &secrets)?;
    let draft = checked(&text, &db, None, &secrets)?;
    let allowed = allowance(&services, widenings(None, &draft))?;
    let id = mcp::store_in(&open.repo_root, None, draft, &allowed).map_err(|error| save_refusal(&error))?;
    services.changes.record(open.project.id(), Outside::Connections);
    server_result(&open.repo_root, id)
  })
  .await
}

pub async fn update_mcp_server(services: &Services, args: UpdateMcpServer) -> Result<Value, ToolError> {
  if let Some(found) = team_mcp_servers::named(services, &args.project, &args.server).await? {
    return team_mcp_servers::update(services, found, args).await;
  }
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let stored = server_named(&db, &args.server)?;
    let secrets = db.secrets()?;
    let text = filled(McpServerText::of(&stored, &secrets), args.fields, &secrets)?;
    let draft = checked(&text, &db, Some(&stored), &secrets)?;
    let allowed = allowance(&services, widenings(Some(&stored), &draft))?;
    mcp::store_in(&open.repo_root, Some(stored.id()), draft, &allowed).map_err(|error| save_refusal(&error))?;
    services.changes.record(open.project.id(), Outside::Connections);
    server_result(&open.repo_root, stored.id())
  })
  .await
}

pub async fn remove_mcp_server(services: &Services, args: InServer) -> Result<Value, ToolError> {
  if let Some(found) = team_mcp_servers::named(services, &args.project, &args.server).await? {
    return team_mcp_servers::remove(services, found).await;
  }
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let server = server_named(&ProjectDb::open(&open.repo_root)?, &args.server)?;
    mcp::remove_in(&open.repo_root, server.id()).map_err(|error| save_refusal(&error))?;
    services.changes.record(open.project.id(), Outside::Connections);
    Ok(json!({ "removed": { "id": server.id().to_string(), "name": server.name().as_str() } }))
  })
  .await
}

pub async fn set_mcp_server_enabled(services: &Services, args: SetMcpServerEnabled) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    if let Some(team) = team_resource_named(&db, ResourceKind::McpServer, &args.server)? {
      return team_fill_ins::switch_server(&services, &open.repo_root, &team, args.enabled);
    }
    let server = server_named(&db, &args.server)?;
    widening::allowed(&services, enabling(&server, args.enabled))?;
    mcp::set_enabled_in(&open.repo_root, server.id(), args.enabled)?;
    services.changes.record(open.project.id(), Outside::Connections);
    server_result(&open.repo_root, server.id())
  })
  .await
}

/// The project's server named by `named`: its id, or else its name (a name never reads as an id).
pub(super) fn server_named(db: &ProjectDb, named: &str) -> Result<McpServer, ToolError> {
  let servers = db.mcp_servers()?;
  let by_id = servers.iter().position(|server| server.id().to_string() == named);
  let by_name = || servers.iter().position(|server| server.name().as_str() == named);
  by_id
    .or_else(by_name)
    .map(|index| servers[index].clone())
    .ok_or_else(|| ToolError::not_found(Missing::McpServer, named))
}

/// The server `text` describes, checked as the page checks it; its name not another server's.
pub(super) fn checked(
  text: &McpServerText,
  db: &ProjectDb,
  stored: Option<&McpServer>,
  secrets: &[Secret],
) -> Result<mcp::McpServerDraft, ToolError> {
  let taken: Vec<McpServerName> = db
    .mcp_servers()?
    .into_iter()
    .filter(|server| Some(server.id()) != stored.map(McpServer::id))
    .map(|server| server.name().clone())
    .collect();
  text.draft(&taken, secrets).map_err(|errors| {
    let code = match &errors.name {
      Some(name) if name.key == NAME_TAKEN => "name_taken",
      _ => "invalid_server",
    };
    let reasons: Vec<Message> = errors.in_order().into_iter().flatten().cloned().collect();
    ToolError::refused(code, reasons)
  })
}

/// What a save may widen once [`widening::allowed`] let it: anything while a person lets automation widen, else
/// nothing, also when the server changed meanwhile (`mcp::store_in` checks the server as stored then).
fn allowance(services: &Services, reasons: Vec<Message>) -> Result<Allowed, ToolError> {
  let widens = !reasons.is_empty();
  widening::allowed(services, reasons)?;
  Ok(if widens {
    Allowed::Anything
  } else {
    Allowed::These(Vec::new())
  })
}

pub(super) fn save_refusal(error: &McpSaveError) -> ToolError {
  let code = match error {
    McpSaveError::Program(problem) => match problem {
      ProgramProblem::InWorktree => "program_in_worktree",
      ProgramProblem::NotFound(_) => "program_not_found",
      ProgramProblem::NotLocal(_) => "program_not_local",
      ProgramProblem::InRepository(_) => "program_in_repository",
      ProgramProblem::NotText(_) => "program_path_not_text",
      ProgramProblem::Invalid(_) => "invalid_server",
    },
    McpSaveError::Store(orchester_store::StoreError::DuplicateConnectionName(_)) => "name_taken",
    McpSaveError::Store(orchester_store::StoreError::ConnectionNotFound(_)) => "not_found",
    McpSaveError::InUse(_) => "in_use",
    McpSaveError::Changed(_) => "changed_meanwhile",
    McpSaveError::SecretStore { .. } => "secret_store_refused",
    McpSaveError::SecretGone(_) => "secret_gone",
    McpSaveError::Store(_) | McpSaveError::Server(_) => "project_data",
  };
  let mut reasons = vec![error.message()];
  if let McpSaveError::Changed(widens) = error {
    reasons.extend(widens.iter().cloned());
  }
  ToolError::refused(code, reasons)
}
