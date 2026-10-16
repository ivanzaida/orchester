use thiserror::Error;

use super::SecretValue;
use crate::id::{InstallationId, ProjectId, SecretId, ServerId};

/// Where a secret's value is kept in the OS keychain: the service `orchester:<project id>` and the
/// account `<secret id>`. Both ids are UUIDs that never change, so a project whose folder moves
/// keeps its values; the project database (never committed) is what points at them. A shared
/// server's token is kept the same way under its own service ([`VaultKey::server_token`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VaultKey {
  service: String,
  account: String,
}

impl VaultKey {
  const SERVICE_PREFIX: &str = "orchester:";
  const SERVER_PREFIX: &str = "orchester-server:";
  const TOKEN_ACCOUNT: &str = "token";
  const APP_MCP_SERVICE: &str = "orchester-app-mcp";
  const CLOUD_SERVICE: &str = "orchester-cloud";

  pub fn new(project: ProjectId, secret: SecretId) -> Self {
    Self {
      service: format!("{}{project}", Self::SERVICE_PREFIX),
      account: secret.to_string(),
    }
  }

  /// The token this computer signed in to `server` with: the service `orchester-server:<server
  /// id>` and the account `token` (`docs/architecture/server.md`, "Accounts and tokens").
  pub fn server_token(server: ServerId) -> Self {
    Self {
      service: format!("{}{server}", Self::SERVER_PREFIX),
      account: Self::TOKEN_ACCOUNT.to_owned(),
    }
  }

  /// The token one installation signed in to Orchester Cloud with: the service `orchester-cloud` and the account
  /// `<installation id>` (`docs/architecture/server.md`, "Orchester Cloud"), so a development build or a QA copy
  /// never signs the installed app in or out.
  pub fn cloud_device(installation: InstallationId) -> Self {
    Self {
      service: Self::CLOUD_SERVICE.to_owned(),
      account: installation.to_string(),
    }
  }

  /// The bearer token of `orchester-app`'s MCP server for one installation: the service `orchester-app-mcp`
  /// and the account `<installation id>` (`docs/design/app-mcp-connection.md`, "Security"), so a development
  /// build and the installed app never share a token.
  pub fn app_mcp_token(installation: InstallationId) -> Self {
    Self {
      service: Self::APP_MCP_SERVICE.to_owned(),
      account: installation.to_string(),
    }
  }

  pub fn service(&self) -> &str {
    &self.service
  }

  pub fn account(&self) -> &str {
    &self.account
  }
}

/// Why the keychain did not do what was asked. Details come from the platform's own messages;
/// keychain APIs never echo a value, and no variant carries one.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum VaultError {
  /// This platform has no keychain implementation yet.
  #[error("storing secrets in the system keychain is not supported on this platform yet")]
  Unsupported,
  /// The keychain is locked, or the user or the system refused access.
  #[error("the system keychain refused access: {0}")]
  AccessDenied(String),
  /// The value is longer than the keychain of this platform accepts.
  #[error("the value is longer than the system keychain accepts ({max} bytes at most)")]
  TooLong { max: u32 },
  #[error("the system keychain failed: {0}")]
  Failed(String),
  /// The keychain has no value under the key: it was removed, or never stored on this computer.
  #[error("the system keychain has no value for this secret")]
  NotFound,
}

/// The OS keychain: the only place a secret value is kept (`docs/architecture/storage.md`,
/// "secrets live only in the OS keychain"). The desktop writes and removes values; the runner
/// reads them when an agent starts, for the MCP servers the agent gets.
pub trait SecretVault: Send + Sync {
  /// Stores `value` under `key`, replacing a value stored there before.
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError>;

  /// The value under `key`; [`VaultError::NotFound`] when there is none.
  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError>;

  /// Removes the value under `key`. A key without a value is not an error, so removing twice
  /// succeeds.
  fn delete(&self, key: &VaultKey) -> Result<(), VaultError>;
}
