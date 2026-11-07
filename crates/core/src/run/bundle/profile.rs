//! An agent profile version as a bundle carries it: everything that decides how its agents work, without what binds
//! them to one computer. The connection that runs the profile is the computer's own, and its extra writable folders
//! are that computer's paths; the executing computer binds them itself, so the bundle has neither. A team profile
//! also names the team MCP servers it gives its agents, by server resource id; their versions are pinned beside the
//! profiles ([`super::CanonicalExecutionInputs::mcp_servers`]). A local profile names its computer's own MCP servers,
//! which the bundle never carries, so a local profile names none.

use serde::{Deserialize, Serialize};

use super::BundleError;
use crate::{
  connection::mcp::McpServerSelection,
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, WritableFolders},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, TeamAgentProfile},
};

/// `{role, responsibilities, instructions, model, settings, permissions, mcp_servers?}`: a profile version without its
/// connection and its writable folders. `mcp_servers` are the team MCP servers it names, distinct and in the order the
/// profile lists them; it is left out while empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ProfileFields")]
pub struct ExecutionProfile {
  role: Name,
  responsibilities: String,
  instructions: PromptText,
  model: ModelId,
  settings: ProviderSettings,
  permissions: AgentPermissions,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  mcp_servers: Vec<ResourceId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileFields {
  role: Name,
  responsibilities: String,
  instructions: PromptText,
  model: ModelId,
  settings: ProviderSettings,
  permissions: AgentPermissions,
  #[serde(default)]
  mcp_servers: Vec<ResourceId>,
}

impl TryFrom<ProfileFields> for ExecutionProfile {
  type Error = BundleError;

  fn try_from(fields: ProfileFields) -> Result<Self, BundleError> {
    if !fields.permissions.writable_folders().is_empty() {
      return Err(BundleError::WritableFolders);
    }
    if let Some(repeated) = fields
      .mcp_servers
      .iter()
      .enumerate()
      .find_map(|(at, server)| fields.mcp_servers[..at].contains(server).then_some(*server))
    {
      return Err(BundleError::Duplicate(repeated));
    }
    Ok(Self {
      role: fields.role,
      responsibilities: fields.responsibilities,
      instructions: fields.instructions,
      model: fields.model,
      settings: fields.settings,
      permissions: fields.permissions,
      mcp_servers: fields.mcp_servers,
    })
  }
}

impl ExecutionProfile {
  pub fn of(profile: &AgentProfile) -> Self {
    Self {
      role: profile.role().clone(),
      responsibilities: profile.responsibilities().to_owned(),
      instructions: profile.instructions().clone(),
      model: profile.model().clone(),
      settings: profile.settings().clone(),
      permissions: profile
        .permissions()
        .clone()
        .with_writable_folders(WritableFolders::none()),
      mcp_servers: Vec::new(),
    }
  }

  /// A team profile version as the bundle carries it, which is how the team server builds the bundle: the same fields
  /// as [`Self::of`] and the team MCP servers it names, so a profile that names none pins the same bytes whichever
  /// body held it.
  pub fn of_team(profile: &TeamAgentProfile) -> Self {
    Self {
      role: profile.role().clone(),
      responsibilities: profile.responsibilities().to_owned(),
      instructions: profile.instructions().clone(),
      model: profile.model().clone(),
      settings: profile.settings().clone(),
      permissions: profile
        .permissions()
        .clone()
        .with_writable_folders(WritableFolders::none()),
      mcp_servers: profile.mcp_servers().to_vec(),
    }
  }

  /// The profile as the computer that runs it binds it: on its own `connection`, giving its agents its `servers`, and
  /// with no extra writable folders.
  pub fn bound(&self, connection: ConnectionId, servers: McpServerSelection) -> AgentProfile {
    AgentProfile::new(
      self.role.clone(),
      self.responsibilities.clone(),
      self.instructions.clone(),
      connection,
      self.model.clone(),
      self.settings.clone(),
    )
    .with_permissions(self.permissions.clone())
    .with_mcp_servers(servers)
  }

  pub fn role(&self) -> &Name {
    &self.role
  }

  pub fn responsibilities(&self) -> &str {
    &self.responsibilities
  }

  pub fn instructions(&self) -> &PromptText {
    &self.instructions
  }

  pub fn model(&self) -> &ModelId {
    &self.model
  }

  pub fn settings(&self) -> &ProviderSettings {
    &self.settings
  }

  /// The team MCP servers the profile names, by server resource id.
  pub fn mcp_servers(&self) -> &[ResourceId] {
    &self.mcp_servers
  }

  /// The profile's permissions without writable folders.
  pub const fn permissions(&self) -> &AgentPermissions {
    &self.permissions
  }
}
