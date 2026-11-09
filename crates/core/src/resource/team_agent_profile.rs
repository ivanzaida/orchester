//! The team's agent profile: what the team server holds of one (`docs/design/team-configuration.md`, "Who owns
//! what"). It names the CLI kind it runs with, never a connection; the MCP servers, tools, skills and instructions it
//! uses by their server ids, never an id from a computer; and the secrets it needs by name, never a value or a secret
//! id. The type has no field a local id could be kept in, and its JSON refuses any other field, so a profile written
//! for one computer (`connection`, a local MCP server id) is not a team profile. Each computer fills in the rest: which
//! of its connections runs the CLI kind, which team MCP servers are on, the secrets' values.
//!
//! The profile carries no writable folders either: they are paths of somebody's computer, so a body that has any is
//! refused, as the execution bundle refuses them. A computer keeps its own folders.
//!
//! The local [`AgentProfile`] keeps local ids. [`TeamAgentProfile::from_local`] is the one way a computer turns its
//! profile into the team's, and [`AgentProfile::from_team`] the one way a team's runs on a connection of this computer.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{AgentProfile, ModelId, ProviderSettings, ResourceBody, ResourceKind, prompt::PromptText};
use crate::{
  connection::{
    AgentProvider, Connection, ConnectionKind,
    mcp::{McpServer, McpServerSelection},
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::AgentPermissions,
  secret::SecretName,
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TeamProfileError {
  #[error("{list} lists `{item}` twice")]
  Duplicate { list: &'static str, item: String },
  #[error("a profile lists at most {max} {list}", max = TeamAgentProfile::MAX_LISTED)]
  TooMany { list: &'static str },
  /// The profile's connection is not an agent connection of this computer, so it names no CLI kind.
  #[error("connection {0} is not an agent connection of this computer, so the profile names no CLI kind")]
  Connection(ConnectionId),
  /// The profile chooses an MCP server of this computer that is not a team resource, which the team cannot name.
  #[error("the MCP server `{0}` is not a team resource; only team MCP servers can be named in a team profile")]
  LocalMcpServer(String),
  /// The profile has extra writable folders, which are paths of one computer and stay there.
  #[error("a team profile carries no writable folders; they are paths of one computer ({})", folders.join(", "))]
  WritableFolders { folders: Vec<String> },
}

/// What a computer knows to turn its profile into the team's.
#[derive(Clone, Copy, Debug)]
pub struct TeamCounterparts<'a> {
  /// The connections of this computer, which tell the CLI kind the profile's connection runs.
  pub connections: &'a [Connection],
  /// The MCP servers of this computer, which name the ones with no team counterpart.
  pub local_mcp_servers: &'a [McpServer],
  /// The team's MCP servers, by their server ids.
  pub team_mcp_servers: &'a [ResourceId],
}

/// An agent profile of a team project, as a versioned resource.
///
/// The ids in its lists are the server's: this type only checks that each list is distinct and short (a UUID parses
/// as a [`ResourceId`] of any kind), so the team server checks that each id names an existing, not archived team
/// resource of the right kind. The settings are [`ProviderSettings`], a free map, so a secret is named in
/// [`Self::secrets`] and never put there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "wire::TeamAgentProfileWire", into = "wire::TeamAgentProfileWire")]
pub struct TeamAgentProfile {
  role: Name,
  responsibilities: String,
  instructions: PromptText,
  cli: AgentProvider,
  model: ModelId,
  settings: ProviderSettings,
  permissions: AgentPermissions,
  mcp_servers: Vec<ResourceId>,
  tools: Vec<ResourceId>,
  skills: Vec<ResourceId>,
  instruction_resources: Vec<ResourceId>,
  secrets: Vec<SecretName>,
}

impl TeamAgentProfile {
  /// How many MCP servers, tools, skills, instructions or secrets a profile lists each; the MCP servers are as many
  /// as a local profile chooses ([`McpServerSelection::MAX`]).
  pub const MAX_LISTED: usize = McpServerSelection::MAX;

  /// A profile with the most restrictive permissions ([`AgentPermissions::before_permissions`]) that uses nothing and
  /// needs no secret.
  pub fn new(
    role: Name,
    responsibilities: String,
    instructions: PromptText,
    (cli, model, settings): (AgentProvider, ModelId, ProviderSettings),
  ) -> Self {
    Self {
      role,
      responsibilities,
      instructions,
      cli,
      model,
      settings,
      permissions: AgentPermissions::before_permissions(),
      mcp_servers: Vec::new(),
      tools: Vec::new(),
      skills: Vec::new(),
      instruction_resources: Vec::new(),
      secrets: Vec::new(),
    }
  }

  /// The permissions the profile's agents run with. Refused when they name extra writable folders: those are paths of
  /// one computer, so a team profile never carries them.
  pub fn with_permissions(mut self, permissions: AgentPermissions) -> Result<Self, TeamProfileError> {
    refuse_writable_folders(&permissions)?;
    self.permissions = permissions;
    Ok(self)
  }

  /// The team MCP servers the profile's agents get, by server id, in the order given.
  pub fn with_mcp_servers(mut self, servers: Vec<ResourceId>) -> Result<Self, TeamProfileError> {
    self.mcp_servers = listed("MCP servers", servers)?;
    Ok(self)
  }

  /// The team tools the profile's agents may call, by server id.
  pub fn with_tools(mut self, tools: Vec<ResourceId>) -> Result<Self, TeamProfileError> {
    self.tools = listed("tools", tools)?;
    Ok(self)
  }

  /// The team skills the profile's agents are offered, by server id.
  pub fn with_skills(mut self, skills: Vec<ResourceId>) -> Result<Self, TeamProfileError> {
    self.skills = listed("skills", skills)?;
    Ok(self)
  }

  /// The team instructions the profile's agents are given, by server id.
  pub fn with_instruction_resources(mut self, instructions: Vec<ResourceId>) -> Result<Self, TeamProfileError> {
    self.instruction_resources = listed("instructions", instructions)?;
    Ok(self)
  }

  /// The secrets the profile needs, by name; each computer fills in the values.
  pub fn with_secrets(mut self, secrets: Vec<SecretName>) -> Result<Self, TeamProfileError> {
    self.secrets = listed("secrets", secrets)?;
    Ok(self)
  }

  /// `profile` as the team holds it: the CLI kind of its connection, and its MCP servers by the server id of the team
  /// MCP server each is. The instructions, skills and tools a local profile gives its agents are not carried over (the
  /// team profile lists none; a team run still goes by their scopes, as its execution bundle names no profile lists),
  /// nor are secrets, which a local profile does not name. Fails when the connection is not an agent connection of this
  /// computer, or the profile chooses an MCP server that is not a team one: the team never holds an id from a
  /// computer. A profile with extra writable folders is refused rather than stripped, so nobody believes the team's
  /// profile keeps them.
  pub fn from_local(profile: &AgentProfile, known: TeamCounterparts<'_>) -> Result<Self, TeamProfileError> {
    // Before the connection and MCP lookups, so the folders are what such a profile is refused for.
    refuse_writable_folders(profile.permissions())?;
    let cli = known
      .connections
      .iter()
      .find(|connection| connection.id() == profile.connection())
      .and_then(|connection| match connection.kind() {
        ConnectionKind::Agent(provider) => Some(*provider),
        ConnectionKind::GitHub { .. } => None,
      })
      .ok_or(TeamProfileError::Connection(profile.connection()))?;
    let servers = profile
      .mcp_servers()
      .ids()
      .iter()
      .map(|chosen| {
        known
          .team_mcp_servers
          .iter()
          .find(|team| ConnectionId::from_uuid(team.as_uuid()) == *chosen)
          .copied()
          .ok_or_else(|| TeamProfileError::LocalMcpServer(local_name(known.local_mcp_servers, *chosen)))
      })
      .collect::<Result<_, _>>()?;
    Self::new(
      profile.role().clone(),
      profile.responsibilities().to_owned(),
      profile.instructions().clone(),
      (cli, profile.model().clone(), profile.settings().clone()),
    )
    .with_permissions(profile.permissions().clone())?
    .with_mcp_servers(servers)
  }

  pub fn role(&self) -> &Name {
    &self.role
  }

  pub fn responsibilities(&self) -> &str {
    &self.responsibilities
  }

  /// Role instructions, the first source in prompt assembly.
  pub const fn instructions(&self) -> &PromptText {
    &self.instructions
  }

  /// The CLI kind the profile runs with; each computer runs it on one of its own connections of that kind.
  pub const fn cli(&self) -> AgentProvider {
    self.cli
  }

  pub const fn model(&self) -> &ModelId {
    &self.model
  }

  pub const fn settings(&self) -> &ProviderSettings {
    &self.settings
  }

  pub const fn permissions(&self) -> &AgentPermissions {
    &self.permissions
  }

  /// The team MCP servers by server id.
  pub fn mcp_servers(&self) -> &[ResourceId] {
    &self.mcp_servers
  }

  pub fn tools(&self) -> &[ResourceId] {
    &self.tools
  }

  pub fn skills(&self) -> &[ResourceId] {
    &self.skills
  }

  /// The team instructions the profile uses, apart from its own role [`Self::instructions`].
  pub fn instruction_resources(&self) -> &[ResourceId] {
    &self.instruction_resources
  }

  pub fn secrets(&self) -> &[SecretName] {
    &self.secrets
  }
}

impl AgentProfile {
  /// The connection a team profile has on this computer while it has none: no pick, and not exactly one connection of
  /// its CLI kind. It is the profile's own id, which no connection of this computer has, so nothing resolves it; this
  /// is the only place that stands it in, and [`Self::awaits_connection_pick`] the only place that recognizes it.
  pub fn unresolved_connection(profile: ResourceId) -> ConnectionId {
    ConnectionId::from_uuid(profile.as_uuid())
  }

  /// Whether this body is that of team profile `profile` with no connection yet ([`Self::unresolved_connection`]):
  /// the person has to pick one, which is not a connection that is gone.
  pub fn awaits_connection_pick(&self, profile: ResourceId) -> bool {
    self.connection() == Self::unresolved_connection(profile)
  }

  /// The team's `profile` as it runs on `connection` of this computer. The team MCP servers are chosen as the local
  /// profile chooses them: by their server ids, carried as [`ConnectionId`]s. The team's tools, skills, instruction
  /// resources and secrets stay in the version's server body: a team run takes what its agents get from its execution
  /// bundle, never from this local body.
  pub fn from_team(profile: &TeamAgentProfile, connection: ConnectionId) -> Self {
    let servers = profile
      .mcp_servers()
      .iter()
      .map(|server| ConnectionId::from_uuid(server.as_uuid()))
      .collect();
    Self::new(
      profile.role().clone(),
      profile.responsibilities().to_owned(),
      profile.instructions().clone(),
      connection,
      profile.model().clone(),
      profile.settings().clone(),
    )
    .with_permissions(profile.permissions().clone())
    .with_mcp_servers(
      McpServerSelection::new(servers)
        .expect("a team profile lists distinct MCP servers, at most as many as a local one"),
    )
  }
}

impl ResourceBody for TeamAgentProfile {
  const KIND: ResourceKind = ResourceKind::AgentProfile;
  const SCHEMA: u32 = 1;
}

/// `items` in order, each once, at most [`TeamAgentProfile::MAX_LISTED`].
fn listed<T: PartialEq + ToString>(list: &'static str, items: Vec<T>) -> Result<Vec<T>, TeamProfileError> {
  if items.len() > TeamAgentProfile::MAX_LISTED {
    return Err(TeamProfileError::TooMany { list });
  }
  for (place, item) in items.iter().enumerate() {
    if items[..place].contains(item) {
      return Err(TeamProfileError::Duplicate {
        list,
        item: item.to_string(),
      });
    }
  }
  Ok(items)
}

/// `permissions` must grant no extra writable folder: the team's profile has none.
fn refuse_writable_folders(permissions: &AgentPermissions) -> Result<(), TeamProfileError> {
  let folders = permissions.writable_folders();
  if folders.is_empty() {
    return Ok(());
  }
  Err(TeamProfileError::WritableFolders {
    folders: folders
      .entries()
      .iter()
      .map(|folder| folder.as_str().to_owned())
      .collect(),
  })
}

/// The name of the local MCP server `chosen`, or its id when this computer does not list it.
fn local_name(servers: &[McpServer], chosen: ConnectionId) -> String {
  servers
    .iter()
    .find(|server| server.id() == chosen)
    .map_or_else(|| chosen.to_string(), |server| server.name().as_str().to_owned())
}

mod wire {
  use serde::{Deserialize, Serialize};

  use super::{
    AgentPermissions, AgentProvider, ModelId, Name, PromptText, ProviderSettings, ResourceId, SecretName,
    TeamAgentProfile, TeamProfileError,
  };

  /// The JSON of a team profile. Unknown fields are refused, so `connection` and a local MCP server id have nowhere to
  /// go; the lists left out while empty.
  #[derive(Serialize, Deserialize)]
  #[serde(deny_unknown_fields)]
  pub struct TeamAgentProfileWire {
    role: Name,
    responsibilities: String,
    instructions: PromptText,
    cli: AgentProvider,
    model: ModelId,
    settings: ProviderSettings,
    permissions: AgentPermissions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    mcp_servers: Vec<ResourceId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ResourceId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    skills: Vec<ResourceId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    instruction_resources: Vec<ResourceId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    secrets: Vec<SecretName>,
  }

  impl TryFrom<TeamAgentProfileWire> for TeamAgentProfile {
    type Error = TeamProfileError;

    fn try_from(wire: TeamAgentProfileWire) -> Result<Self, TeamProfileError> {
      Self::new(
        wire.role,
        wire.responsibilities,
        wire.instructions,
        (wire.cli, wire.model, wire.settings),
      )
      .with_permissions(wire.permissions)?
      .with_mcp_servers(wire.mcp_servers)?
      .with_tools(wire.tools)?
      .with_skills(wire.skills)?
      .with_instruction_resources(wire.instruction_resources)?
      .with_secrets(wire.secrets)
    }
  }

  impl From<TeamAgentProfile> for TeamAgentProfileWire {
    fn from(profile: TeamAgentProfile) -> Self {
      Self {
        role: profile.role,
        responsibilities: profile.responsibilities,
        instructions: profile.instructions,
        cli: profile.cli,
        model: profile.model,
        settings: profile.settings,
        permissions: profile.permissions,
        mcp_servers: profile.mcp_servers,
        tools: profile.tools,
        skills: profile.skills,
        instruction_resources: profile.instruction_resources,
        secrets: profile.secrets,
      }
    }
  }
}
