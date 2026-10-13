//! The bearer token clients present on every request, and where an installed app keeps it: the OS secret store
//! (Windows Credential Manager, the macOS login keychain) under this installation's id
//! ([`VaultKey::app_mcp_token`]), never a file, the database, a log or the MCP tree. A QA copy, a test or a
//! development build makes a new one on each start and keeps it in memory and its discovery file only.

use std::sync::Arc;

use orchester_core::{
  id::InstallationId,
  secret::{SecretValue, VaultError, VaultKey},
};
use orchester_workspace::secrets::Vault;

/// The bearer token clients present. It is a secret: it shows in no `Debug` output and has no
/// `Display`.
#[derive(Clone)]
pub struct BearerToken(Arc<str>);

impl std::fmt::Debug for BearerToken {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("BearerToken(<redacted>)")
  }
}

impl BearerToken {
  /// A new token from the OS random source: two random UUIDs, 244 random bits.
  pub fn generate() -> Self {
    Self(format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple()).into())
  }

  /// The token itself, for the discovery file, the secret store, and the window's Show and Copy on a
  /// person's click.
  pub fn expose(&self) -> &str {
    &self.0
  }

  /// Whether `presented` is this token, compared in constant time so the comparison does not tell how
  /// much of it matched.
  pub(super) fn admits(&self, presented: &str) -> bool {
    let (expected, presented) = (self.0.as_bytes(), presented.as_bytes());
    expected.len() == presented.len()
      && expected
        .iter()
        .zip(presented)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
  }
}

/// An installation's token in the OS secret store.
#[derive(Clone)]
pub struct TokenStore {
  vault: Vault,
  key: VaultKey,
}

impl TokenStore {
  pub fn new(vault: Vault, installation: InstallationId) -> Self {
    Self {
      vault,
      key: VaultKey::app_mcp_token(installation),
    }
  }

  /// The stored token; when there is none yet, a new one, stored first.
  pub fn read_or_create(&self) -> Result<BearerToken, VaultError> {
    match self.vault.get().read(&self.key) {
      Ok(stored) => Ok(BearerToken(stored.expose().into())),
      Err(VaultError::NotFound) => self.replace(),
      Err(error) => Err(error),
    }
  }

  /// A new token, stored in place of the old one; the old one stays stored when this fails.
  pub fn replace(&self) -> Result<BearerToken, VaultError> {
    let token = BearerToken::generate();
    let value = SecretValue::new(token.expose().to_owned()).expect("a generated token is 64 hex digits, never blank");
    self.vault.get().store(&self.key, &value)?;
    Ok(token)
  }
}
