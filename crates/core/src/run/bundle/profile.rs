//! An agent profile version as a bundle carries it: everything that decides how its agents work, without what binds
//! them to one computer. The connection that runs the profile and the MCP servers it gives its agents are the
//! computer's own connections, and its extra writable folders are that computer's paths; the executing computer binds
//! them itself, so the bundle has none of them.

use serde::{Deserialize, Serialize};

use super::BundleError;
use crate::{
  connection::mcp::McpServerSelection,
  id::ConnectionId,
  name::Name,
  permission::{AgentPermissions, WritableFolders},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings},
};

/// `{role, responsibilities, instructions, model, settings, permissions}`: a profile version without its connection,
/// its MCP servers and its writable folders.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ProfileFields")]
pub struct ExecutionProfile {
  role: Name,
  responsibilities: String,
  instructions: PromptText,
  model: ModelId,
  settings: ProviderSettings,
  permissions: AgentPermissions,
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
}

impl TryFrom<ProfileFields> for ExecutionProfile {
  type Error = BundleError;

  fn try_from(fields: ProfileFields) -> Result<Self, BundleError> {
    if !fields.permissions.writable_folders().is_empty() {
      return Err(BundleError::WritableFolders);
    }
    Ok(Self {
      role: fields.role,
      responsibilities: fields.responsibilities,
      instructions: fields.instructions,
      model: fields.model,
      settings: fields.settings,
      permissions: fields.permissions,
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

  /// The profile's permissions without writable folders.
  pub const fn permissions(&self) -> &AgentPermissions {
    &self.permissions
  }
}
