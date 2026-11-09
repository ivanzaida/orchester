use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{chosen::ChosenResources, prompt::PromptText};
use crate::{
  connection::mcp::McpServerSelection,
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::AgentPermissions,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelIdError {
  #[error(
    "model id must be 1–{max} characters without whitespace, got `{0}`",
    max = ModelId::MAX_CHARS
  )]
  Invalid(String),
}

/// A model identifier as listed by the connection's provider catalog. Orchester does not know
/// the provider vocabularies, so any non-blank token is accepted here; compatibility with the
/// connection is checked against the adapter's catalog, not by this type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ModelId(String);

impl ModelId {
  pub const MAX_CHARS: usize = 200;

  pub fn new(value: &str) -> Result<Self, ModelIdError> {
    let length = value.chars().count();
    let valid = (1..=Self::MAX_CHARS).contains(&length) && !value.chars().any(|c| c.is_whitespace() || c.is_control());
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(ModelIdError::Invalid(value.to_owned()))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for ModelId {
  type Error = ModelIdError;

  fn try_from(value: String) -> Result<Self, ModelIdError> {
    Self::new(&value)
  }
}

impl From<ModelId> for String {
  fn from(model: ModelId) -> Self {
    model.0
  }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderSettingsError {
  #[error("provider setting names must be non-blank and without whitespace, got `{0}`")]
  InvalidName(String),
}

/// Provider-specific settings (for example reasoning effort) passed through to the adapter
/// unchanged. Orchester does not interpret them; the adapter validates them. It is a free map that is stored, shared
/// with the team and bundled as it is, so no secret value belongs in it: a profile needs a secret by its name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, String>", into = "BTreeMap<String, String>")]
pub struct ProviderSettings(BTreeMap<String, String>);

impl ProviderSettings {
  pub fn new(settings: BTreeMap<String, String>) -> Result<Self, ProviderSettingsError> {
    let invalid = settings
      .keys()
      .find(|name| name.is_empty() || name.chars().any(|c| c.is_whitespace() || c.is_control()));
    match invalid {
      Some(name) => Err(ProviderSettingsError::InvalidName(name.clone())),
      None => Ok(Self(settings)),
    }
  }

  /// Settings by name, in name order.
  pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
    self.0.iter().map(|(name, value)| (name.as_str(), value.as_str()))
  }
}

impl TryFrom<BTreeMap<String, String>> for ProviderSettings {
  type Error = ProviderSettingsError;

  fn try_from(settings: BTreeMap<String, String>) -> Result<Self, ProviderSettingsError> {
    Self::new(settings)
  }
}

impl From<ProviderSettings> for BTreeMap<String, String> {
  fn from(settings: ProviderSettings) -> Self {
    settings.0
  }
}

/// A project agent role: what it is responsible for, how it is instructed, which connection and
/// model run it, what it may do and what its agents get: MCP servers, and the project's instructions, skills and tools
/// it chooses (`docs/design/team-configuration.md`, "Decided 2026-11-09"; nothing else gives an agent of a local run
/// one). Pipeline steps select a profile, inherit its model and what it gives and can only narrow its permissions.
/// The three lists are left out of the stored body while empty, so a profile that chooses none keeps its shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
  role: Name,
  responsibilities: String,
  instructions: PromptText,
  connection: ConnectionId,
  model: ModelId,
  settings: ProviderSettings,
  permissions: AgentPermissions,
  mcp_servers: McpServerSelection,
  #[serde(default, skip_serializing_if = "ChosenResources::is_empty")]
  instruction_resources: ChosenResources,
  #[serde(default, skip_serializing_if = "ChosenResources::is_empty")]
  skills: ChosenResources,
  #[serde(default, skip_serializing_if = "ChosenResources::is_empty")]
  tools: ChosenResources,
}

impl AgentProfile {
  /// A profile with the most restrictive permissions ([`AgentPermissions::before_permissions`]) that gives its agents
  /// nothing; see [`Self::with_permissions`], [`Self::with_mcp_servers`] and [`Self::with_given`].
  pub const fn new(
    role: Name,
    responsibilities: String,
    instructions: PromptText,
    connection: ConnectionId,
    model: ModelId,
    settings: ProviderSettings,
  ) -> Self {
    Self {
      role,
      responsibilities,
      instructions,
      connection,
      model,
      settings,
      permissions: AgentPermissions::before_permissions(),
      mcp_servers: McpServerSelection::none(),
      instruction_resources: ChosenResources::EMPTY,
      skills: ChosenResources::EMPTY,
      tools: ChosenResources::EMPTY,
    }
  }

  #[must_use]
  pub fn with_permissions(mut self, permissions: AgentPermissions) -> Self {
    self.permissions = permissions;
    self
  }

  /// The profile run on `connection` instead.
  #[must_use]
  pub const fn with_connection(mut self, connection: ConnectionId) -> Self {
    self.connection = connection;
    self
  }

  #[must_use]
  pub fn with_mcp_servers(mut self, servers: McpServerSelection) -> Self {
    self.mcp_servers = servers;
    self
  }

  /// The profile giving its agents `instructions`, `skills` and `tools` of the project, in place of what it gave.
  #[must_use]
  pub fn with_given(mut self, instructions: ChosenResources, skills: ChosenResources, tools: ChosenResources) -> Self {
    self.instruction_resources = instructions;
    self.skills = skills;
    self.tools = tools;
    self
  }

  pub fn role(&self) -> &Name {
    &self.role
  }

  pub fn responsibilities(&self) -> &str {
    &self.responsibilities
  }

  /// Role instructions, the first source in prompt assembly.
  pub fn instructions(&self) -> &PromptText {
    &self.instructions
  }

  pub const fn connection(&self) -> ConnectionId {
    self.connection
  }

  pub fn model(&self) -> &ModelId {
    &self.model
  }

  pub fn settings(&self) -> &ProviderSettings {
    &self.settings
  }

  pub const fn permissions(&self) -> &AgentPermissions {
    &self.permissions
  }

  /// The MCP servers the profile's agents get; none by default.
  pub const fn mcp_servers(&self) -> &McpServerSelection {
    &self.mcp_servers
  }

  /// The project's instructions its agents are given, apart from its own role [`Self::instructions`].
  pub const fn instruction_resources(&self) -> &ChosenResources {
    &self.instruction_resources
  }

  /// The project's skills its agents are offered.
  pub const fn skills(&self) -> &ChosenResources {
    &self.skills
  }

  /// The project's tools its agents may call.
  pub const fn tools(&self) -> &ChosenResources {
    &self.tools
  }

  /// Whether the profile chooses `resource` as an instruction, skill or tool.
  pub fn gives(&self, resource: ResourceId) -> bool {
    self.instruction_resources.contains(resource) || self.skills.contains(resource) || self.tools.contains(resource)
  }
}
