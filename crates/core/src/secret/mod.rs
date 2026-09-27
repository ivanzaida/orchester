//! Project secrets (`docs/design/orchester-workspace.md`, "Secrets and access"). A secret belongs
//! to its project: its metadata is a row of the project database ([`crate::store::SecretStore`]),
//! its value lives only in the OS keychain ([`SecretVault`]), under the project and secret ids.
//! The value never enters a database, a log, a prompt or a `Debug` output ([`SecretValue`]).
//! Which tools use a secret is derived from the tools themselves ([`tool_uses`]); the secret
//! stores no tool list, and access rules are edited on the tool.

mod kind;
mod lifecycle;
mod name;
mod value;
mod vault;

pub use self::{
  kind::{Environment, SecretKind, UnknownValue},
  lifecycle::{SecretError, add, archive, existing, replace_value},
  name::{SecretName, SecretNameError},
  value::{SecretValue, SecretValueError},
  vault::{SecretVault, VaultError, VaultKey},
};
use crate::{
  id::{ProjectId, SecretId},
  resource::{HttpAuth, Tool, ToolAction},
  time::Timestamp,
};

/// What the user enters to add a secret, besides its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewSecret {
  pub name: SecretName,
  pub kind: SecretKind,
  pub environment: Environment,
}

/// A project secret's metadata. Its value is in the keychain under [`Secret::vault_key`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Secret {
  id: SecretId,
  name: SecretName,
  kind: SecretKind,
  environment: Environment,
  created_at: Timestamp,
  rotated_at: Option<Timestamp>,
  archived_at: Option<Timestamp>,
}

impl Secret {
  /// A secret added now, whose value was never replaced.
  pub fn new(id: SecretId, new: NewSecret, created_at: Timestamp) -> Self {
    Self::from_stored(id, new, created_at, None, None)
  }

  pub fn from_stored(
    id: SecretId,
    new: NewSecret,
    created_at: Timestamp,
    rotated_at: Option<Timestamp>,
    archived_at: Option<Timestamp>,
  ) -> Self {
    Self {
      id,
      name: new.name,
      kind: new.kind,
      environment: new.environment,
      created_at,
      rotated_at,
      archived_at,
    }
  }

  pub const fn id(&self) -> SecretId {
    self.id
  }

  pub fn name(&self) -> &SecretName {
    &self.name
  }

  pub const fn kind(&self) -> SecretKind {
    self.kind
  }

  pub const fn environment(&self) -> Environment {
    self.environment
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  /// When the value was last replaced; `None` while it is the value the secret was added with.
  pub const fn rotated_at(&self) -> Option<Timestamp> {
    self.rotated_at
  }

  /// When the secret was archived. An archived secret has no value in the keychain and can no
  /// longer be used; its row stays so references to it resolve to an archived secret.
  pub const fn archived_at(&self) -> Option<Timestamp> {
    self.archived_at
  }

  pub const fn is_archived(&self) -> bool {
    self.archived_at.is_some()
  }

  /// Where the value is kept in the keychain of this computer.
  pub fn vault_key(&self, project: ProjectId) -> VaultKey {
    VaultKey::new(project, self.id)
  }

  fn record_rotation(&mut self, at: Timestamp) {
    self.rotated_at = Some(at);
  }

  fn archive(&mut self, at: Timestamp) {
    self.archived_at = Some(at);
  }
}

/// Whether `tool` refers to `secret`: as the bearer token of an HTTP request or the credentials of
/// a database query. Command tools hold no secrets.
pub fn tool_uses(tool: &Tool, secret: SecretId) -> bool {
  match tool.action() {
    ToolAction::Http(request) => request.auth() == HttpAuth::Bearer { secret },
    ToolAction::DatabaseQuery(query) => query.credentials() == secret,
    ToolAction::Command(_) => false,
  }
}
