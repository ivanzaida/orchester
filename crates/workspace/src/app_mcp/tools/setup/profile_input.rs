//! `save_profile_draft`: an agent profile's editor through Orchester's MCP server. Every field is
//! checked as the editor checks it (`agents::form`, `agents::permissions`, `agents::folders`), and only
//! what the editor offers is taken: permissions the connection's CLI enforces on this computer, the
//! project's MCP servers (adding only one that is turned on), an effort the model lists, and no approval switches or
//! MCP servers on a connection whose agents Orchester runs itself. Permissions and MCP servers broader than the
//! profile's latest published version (before its first: the most restrictive its CLI enforces, no server) are a
//! person's decision ([`widening`]); narrowing them is always taken.

use std::path::Path;

use orchester_core::{
  connection::{
    AgentConnection,
    invocation::Platform,
    mcp::{McpServer, McpServerSelection},
  },
  id::ConnectionId,
  permission::{
    AgentPermissions, AllowedCommands, ApprovalMode, BrowserPermission, CommandPermission, NetworkAccess,
    WritableFolders,
    browser::{AllowedSites, BrowserAccess},
  },
  resource::{AgentProfile, ModelId, RepositoryAccess},
  store::McpServerStore,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  connections::agent_named,
  profiles::{profile_named, profile_with_revision},
  widening,
};
use crate::{
  agents::{
    data::{ProfileChange, ProjectAgents, apply_in},
    folders::with_folder,
    form::{ModelSelection, ProfileForm, effort_options, listed_models},
    permissions::{not_enforceable, with_command, with_site},
    servers,
  },
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  drafts::save_draft_expecting_in,
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
  /// The profile's `revision` as get_profile or the last save returned it: the draft is saved only while it
  /// is unchanged, else refused as draft_changed. Left out, the save goes over a change saved meanwhile.
  pub expected_revision: Option<String>,
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
  /// Who answers tool requests; left out, manual. Switching to autopilot is a person's decision.
  #[serde(default)]
  pub approval_mode: Approvals,
  /// The browser: Orchester's browser tools for its agents. Left out, a profile being changed keeps its own and a
  /// new profile has it off.
  pub browser: Option<Browser>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct Browser {
  /// on: its agents get the browser tools, which needs network on.
  pub access: Network,
  /// Hosts its agents open and act on without asking, e.g. docs.rs or *.example.com; any other site asks a person.
  /// Kept while the browser is off.
  #[serde(default)]
  pub allowed_sites: Vec<String>,
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

/// Off or on, as the tools name network access and the browser.
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

/// The approval mode, as the tools name it.
#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Approvals {
  /// Every tool request in the worktree is approved at once; work outside it, pushes and other refs ask a person.
  Autopilot,
  /// The allowed commands run at once; anything else goes to the reviewer, then to a person.
  #[default]
  Manual,
}

impl From<Approvals> for ApprovalMode {
  fn from(mode: Approvals) -> Self {
    match mode {
      Approvals::Autopilot => Self::Autopilot,
      Approvals::Manual => Self::Manual,
    }
  }
}

pub async fn save_profile_draft(services: &Services, args: SaveProfileDraft) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root.clone();
    let agents = ProjectAgents::load_in(&root)?;
    let db = ProjectDb::open(&root)?;
    let existing = args
      .profile
      .as_deref()
      .map(|named| profile_named(&agents, named))
      .transpose()?;
    let connection = agent_named(&db, &args.connection)?;
    let permissions = match &args.permissions {
      Some(given) => {
        let kept = existing.map_or_else(BrowserPermission::off, |entry| {
          entry.profile.permissions().browser().clone()
        });
        permissions(given, &root, kept)?
      }
      None => existing.map_or_else(
        || AgentPermissions::most_restrictive(connection.provider(), Platform::current()),
        |entry| entry.profile.permissions().clone(),
      ),
    };
    offered(&connection, &permissions, &args)?;
    let project_servers = db.mcp_servers()?;
    let servers = mcp_servers(&project_servers, &args.mcp_servers)?;
    let profile = form(&args, &connection, &permissions, &servers)?;
    servers::check(&project_servers, existing.map(|entry| &entry.profile), &profile)
      .map_err(|reason| refused("mcp_server_not_offered", reason))?;
    let id = existing.map(|entry| entry.id);
    widening::check_profile(&services, &db, (id, &profile), connection.provider())?;
    let id = match existing {
      Some(entry) => {
        save_draft_expecting_in(&root, entry.id, profile, args.expected_revision.as_deref())?;
        entry.id
      }
      None if args.expected_revision.is_some() => {
        return Err(ToolError::InvalidArguments(
          "expected_revision names a profile's draft; a new profile has none".into(),
        ));
      }
      None => apply_in(&root, ProfileChange::Create(profile))?,
    };
    services
      .changes
      .record(open.project.id(), crate::tasks::outside::Outside::Profiles);
    Ok(json!({ "profile": profile_with_revision(&open, id)? }))
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
    .filter(|id| listed_models(connection).contains(id));
  // A connection on an API provider takes the effort its CLI does (`ProfileForm`), not a listed one.
  if let (Some(effort), Some(id)) = (&args.effort, &listed)
    && connection.api_provider().is_none()
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
    let reasons = [
      errors.role,
      errors.instructions,
      errors.connection,
      errors.model,
      errors.effort,
    ];
    ToolError::refused("invalid_profile", reasons.into_iter().flatten().collect())
  })
}

/// The permissions given, each command, folder and site checked as the editor adds it; `kept` is the browser
/// permission when none is given.
fn permissions(given: &Permissions, root: &Path, kept: BrowserPermission) -> Result<AgentPermissions, ToolError> {
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
  let browser = match &given.browser {
    Some(browser) => browser_permission(browser, &network).map_err(invalid)?,
    None => kept,
  };
  let mut permissions =
    AgentPermissions::new(given.repository.into(), commands, network).with_writable_folders(folders);
  permissions.set_interactive_tool_approvals(given.ask_before_tool_calls);
  permissions.set_browser(browser);
  permissions.set_approval_mode(given.approval_mode.into());
  Ok(permissions)
}

/// The browser permission given, each site checked as the editor adds it; on needs network on.
fn browser_permission(given: &Browser, network: &NetworkAccess) -> Result<BrowserPermission, Message> {
  let access = match given.access {
    Network::Off => BrowserAccess::Off,
    Network::On if !matches!(network, NetworkAccess::On) => {
      return Err(Message::new("appMcp.errors.browserNeedsNetwork"));
    }
    Network::On => BrowserAccess::On,
  };
  let mut sites = AllowedSites::none();
  for site in &given.allowed_sites {
    sites = with_site(&sites, site)?;
  }
  Ok(BrowserPermission::new(access, sites))
}

/// Refused where the editor offers no such choice on `connection`: permissions its CLI does not
/// enforce here, and approval switches or MCP servers on a connection whose agents Orchester runs.
fn offered(
  connection: &AgentConnection,
  permissions: &AgentPermissions,
  args: &SaveProfileDraft,
) -> Result<(), ToolError> {
  let provider = connection.provider();
  if let Err(problem) = permissions.enforced_on(provider, Platform::current()) {
    return Err(refused("permissions_not_enforced", not_enforceable(&problem)));
  }
  let switches = permissions.interactive_tool_approvals() || !args.mcp_servers.is_empty();
  if provider.is_native() && switches {
    return Err(refused("not_offered", Message::new("appMcp.errors.nativeNoSwitches")));
  }
  Ok(())
}

/// The project's MCP servers `named` names, by id or name.
fn mcp_servers(servers: &[McpServer], named: &[String]) -> Result<McpServerSelection, ToolError> {
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
