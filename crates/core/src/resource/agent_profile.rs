use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::prompt::PromptText;
use crate::{connection::mcp::McpServerSelection, id::ConnectionId, name::Name, permission::AgentPermissions};

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
/// unchanged. Orchester does not interpret them; the adapter validates them.
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
/// model run it, what it may do and which MCP servers its agents get. Pipeline steps select a profile,
/// inherit its model and its MCP servers and can only narrow its permissions.
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
}

impl AgentProfile {
  /// A profile with the most restrictive permissions ([`AgentPermissions::before_permissions`]) and no MCP
  /// servers; see [`Self::with_permissions`] and [`Self::with_mcp_servers`].
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
    }
  }

  #[must_use]
  pub fn with_permissions(mut self, permissions: AgentPermissions) -> Self {
    self.permissions = permissions;
    self
  }

  #[must_use]
  pub fn with_mcp_servers(mut self, servers: McpServerSelection) -> Self {
    self.mcp_servers = servers;
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
}
