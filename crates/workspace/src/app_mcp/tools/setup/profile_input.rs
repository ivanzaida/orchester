//! `save_profile_draft`: an agent profile's editor through Orchester's MCP server. Every field is
//! checked as the editor checks it (`agents::form`, `agents::permissions`, `agents::folders`), and only
//! what the editor offers is taken: permissions the connection's CLI enforces on this computer, the
//! project's MCP servers, an effort the model lists, and no approval switches or MCP servers on a
//! connection whose agents Orchester runs itself. Permissions and MCP servers broader than the profile's
//! latest published version (before its first: the most restrictive its CLI enforces, no server) are a
//! person's decision ([`widening`]); narrowing them is always taken.

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, invocation::Platform, mcp::McpServerSelection},
  id::ConnectionId,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, WritableFolders},
  resource::{AgentProfile, ModelId, RepositoryAccess},
  store::McpServerStore,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  connections::agent_named,
  profiles::{profile_json, profile_named},
  widening,
};
use crate::{
  agents::{
    data::{ProfileChange, ProjectAgents, apply_in},
    folders::with_folder,
    form::{ModelSelection, ProfileForm, effort_options},
    permissions::with_command,
  },
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SaveProfileDraft {
  /// The project's id, or its name.
  pub project: String,
  /// The profile to change, by its id or role; left out, a new profile is made.
  pub profile: Option<String>,
  /// The agent's role, e.g. Builder; pipeline steps and instructions name it.
  pub role: String,
  /// What the agent is responsible for, in a sentence or two.
  #[serde(default)]
  pub responsibilities: String,
  /// The role instructions every prompt of the agent carries; may use {{variables}}.
  #[serde(default)]
  pub instructions: String,
  /// The agent connection it runs on, by its id or name.
  pub connection: String,
  /// The model id, one list_models gives or another the CLI runs.
  pub model: String,
  /// A reasoning effort the model lists; left out, the CLI's default.
  pub effort: Option<String>,
  /// Left out: a new profile gets the most restrictive permissions its CLI enforces, a profile being
  /// changed keeps its own.
  pub permissions: Option<Permissions>,
  /// MCP servers of the project its agents get, by id or name.
  #[serde(default)]
  pub mcp_servers: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct Permissions {
  pub repository: Access,
  /// Folders outside the worktree its agents may write, with write access only.
  #[serde(default)]
  pub writable_folders: Vec<String>,
  pub commands: Commands,
  /// The command prefixes `allowed` lets run, e.g. "cargo test".
  #[serde(default)]
  pub allowed_commands: Vec<String>,
  pub network: Network,
  /// Ask a person before each MCP or app tool call.
  #[serde(default)]
  pub ask_before_tool_calls: bool,
  /// Approve automatically (don't ask me): Orchester answers what would ask a person.
  #[serde(default)]
  pub approve_automatically: bool,
}

/// Repository access, as the tools name it.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Access {
  ReadOnly,
  Write,
}

impl From<Access> for RepositoryAccess {
  fn from(access: Access) -> Self {
    match access {
      Access::ReadOnly => Self::ReadOnly,
      Access::Write => Self::Write,
    }
  }
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Network {
  Off,
  On,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Commands {
  AskBefore,
  Allowed,
  AnyInWorktree,
}

pub async fn save_profile_draft(services: &Services, args: SaveProfileDraft) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root;
    let agents = ProjectAgents::load_in(&root)?;
    let db = ProjectDb::open(&root)?;
    let existing = args
      .profile
      .as_deref()
      .map(|named| profile_named(&agents, named))
      .transpose()?;
    let connection = agent_named(&db, &args.connection)?;
    let permissions = match &args.permissions {
      Some(given) => permissions(given, &root)?,
      None => existing.map_or_else(
        || AgentPermissions::most_restrictive(connection.provider(), Platform::current()),
        |entry| entry.profile.permissions().clone(),
      ),
    };
    offered(&connection, &permissions, &args)?;
    let servers = mcp_servers(&db, &args.mcp_servers)?;
    let profile = form(&args, &connection, &permissions, &servers)?;
    let id = existing.map(|entry| entry.id);
    widening::check_profile(&services, &db, (id, &profile), connection.provider())?;
    let change = match existing {
      Some(entry) => ProfileChange::SaveDraft(entry.id, profile),
      None => ProfileChange::Create(profile),
    };
    let id = apply_in(&root, change)?;
    let agents = ProjectAgents::load_in(&root)?;
    let entry = agents
      .profile(id)
      .ok_or_else(|| ToolError::not_found(Missing::Profile, id.to_string()))?;
    Ok(json!({ "profile": profile_json(&agents, entry) }))
  })
  .await
}

/// The profile the editor's fields make, refused with every field's reason.
fn form(
  args: &SaveProfileDraft,
  connection: &AgentConnection,
  permissions: &AgentPermissions,
  servers: &McpServerSelection,
) -> Result<AgentProfile, ToolError> {
  let model = args.model.trim();
  let listed = ModelId::new(model)
    .ok()
    .filter(|id| connection.models().iter().any(|listed| listed.id() == id));
  if let (Some(effort), Some(id)) = (&args.effort, &listed)
    && !effort_options(connection, id).iter().any(|offered| offered == effort)
  {
    let reason = Message::new("appMcp.errors.effortNotOffered")
      .with("effort", effort.clone())
      .with("model", model.to_owned());
    return Err(refused("effort_not_offered", reason));
  }
  let selection = listed.map_or(ModelSelection::Custom, ModelSelection::Listed);
  ProfileForm {
    role: &args.role,
    responsibilities: &args.responsibilities,
    instructions: &args.instructions,
    connection: Some(connection),
    model: &selection,
    custom_model: model,
    effort: args.effort.as_deref(),
    permissions,
    mcp_servers: servers,
  }
  .profile()
  .map_err(|errors| {
    let reasons = [errors.role, errors.instructions, errors.connection, errors.model];
    ToolError::refused("invalid_profile", reasons.into_iter().flatten().collect())
  })
}

/// The permissions given, each command and folder checked as the editor adds it.
fn permissions(given: &Permissions, root: &Path) -> Result<AgentPermissions, ToolError> {
  let invalid = |reason| refused("invalid_permissions", reason);
  let commands = match given.commands {
    Commands::AskBefore => CommandPermission::AskBefore,
    Commands::AnyInWorktree => CommandPermission::AnyInWorktree,
    Commands::Allowed => {
      let mut list = AllowedCommands::default();
      for command in &given.allowed_commands {
        list = with_command(&list, command).map_err(invalid)?;
      }
      CommandPermission::Allowed(list)
    }
  };
  if !matches!(given.commands, Commands::Allowed) && !given.allowed_commands.is_empty() {
    return Err(invalid(Message::new("appMcp.errors.commandsNeedAllowed")));
  }
  if given.repository == Access::ReadOnly && !given.writable_folders.is_empty() {
    return Err(invalid(Message::new("appMcp.errors.foldersNeedWrite")));
  }
  let mut folders = WritableFolders::none();
  for folder in &given.writable_folders {
    folders = with_folder(&folders, folder, root).map_err(invalid)?;
  }
  let network = match given.network {
    Network::Off => NetworkAccess::Off,
    Network::On => NetworkAccess::On,
  };
  let mut permissions =
    AgentPermissions::new(given.repository.into(), commands, network).with_writable_folders(folders);
  permissions.set_interactive_tool_approvals(given.ask_before_tool_calls);
  permissions.set_auto_approve(given.approve_automatically);
  Ok(permissions)
}

/// Refused where the editor offers no such choice on `connection`: permissions its CLI does not
/// enforce here, and approval switches or MCP servers on a connection whose agents Orchester runs.
fn offered(
  connection: &AgentConnection,
  permissions: &AgentPermissions,
  args: &SaveProfileDraft,
) -> Result<(), ToolError> {
  let provider = connection.provider();
  if permissions.enforced_on(provider, Platform::current()).is_err() {
    return Err(refused(
      "permissions_not_enforced",
      Message::new("agents.permissions.publish.notEnforceable"),
    ));
  }
  let switches = permissions.interactive_tool_approvals() || permissions.auto_approve() || !args.mcp_servers.is_empty();
  if provider.is_native() && switches {
    return Err(refused("not_offered", Message::new("appMcp.errors.nativeNoSwitches")));
  }
  Ok(())
}

/// The project's MCP servers `named` names, by id or name.
fn mcp_servers(db: &ProjectDb, named: &[String]) -> Result<McpServerSelection, ToolError> {
  let servers = db.mcp_servers()?;
  let ids = named
    .iter()
    .map(|name| {
      servers
        .iter()
        .find(|server| {
          server.id().to_string() == *name || server.name().to_string().to_lowercase() == name.to_lowercase()
        })
        .map(|server| server.id())
        .ok_or_else(|| ToolError::not_found(Missing::McpServer, name))
    })
    .collect::<Result<Vec<ConnectionId>, _>>()?;
  McpServerSelection::new(ids).map_err(|_| {
    let reason = Message::new("appMcp.errors.mcpServersInvalid").with("max", McpServerSelection::MAX.to_string());
    refused("invalid_mcp_servers", reason)
  })
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
