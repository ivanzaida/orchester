//! Keeping secret values out of memory. A record reaches agents' prompts, so before one is stored
//! its text, key and tags are checked against the values of the project's secrets; one that holds
//! a value is refused. The refusal names the secret, never the value.
//!
//! Only the value as it is stored is looked for: an encoded value (base64, URL encoding, hex) or
//! one split across lines or words is not detected.

use thiserror::Error;
use zeroize::Zeroizing;

use super::MemoryContent;
use crate::{
  id::ProjectId,
  secret::{Secret, SecretName, SecretValue, SecretVault, VaultError},
};

/// The content holds the value of secret `secret`; it was not stored.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("the memory text contains the value of secret `{secret}`; it was not saved")]
pub struct ContainsSecret {
  pub secret: SecretName,
}

/// Why the project's secret values could not be read to check a record against them.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("cannot read the value of secret `{secret}` to keep it out of memory: {source}")]
pub struct ScreenUnavailable {
  pub secret: SecretName,
  pub source: VaultError,
}

/// Content that holds no secret value of the project, the only content a record can be made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenedContent(MemoryContent);

impl ScreenedContent {
  pub const fn content(&self) -> &MemoryContent {
    &self.0
  }

  pub(super) fn into_content(self) -> MemoryContent {
    self.0
  }
}

/// The values of a project's secrets, to check content against.
pub struct SecretScreen {
  values: Vec<(SecretName, SecretValue)>,
}

impl SecretScreen {
  /// Values shorter than this many characters are looked for as whole tokens only.
  pub const SHORT: usize = 8;

  pub const fn new(values: Vec<(SecretName, SecretValue)>) -> Self {
    Self { values }
  }

  /// The values of `secrets` of `project` from `vault`. A secret with no value in the keychain
  /// has nothing to leak and is skipped; any other failure means content cannot be checked.
  pub fn load(project: ProjectId, secrets: &[Secret], vault: &dyn SecretVault) -> Result<Self, ScreenUnavailable> {
    let mut values = Vec::with_capacity(secrets.len());
    for secret in secrets.iter().filter(|secret| !secret.is_archived()) {
      match vault.read(&secret.vault_key(project)) {
        Ok(value) => values.push((secret.name().clone(), value)),
        Err(VaultError::NotFound) => {}
        Err(source) => {
          return Err(ScreenUnavailable {
            secret: secret.name().clone(),
            source,
          });
        }
      }
    }
    Ok(Self { values })
  }

  /// `content`, if neither its text, its key nor its tags hold a secret's value. A value is looked
  /// for without the whitespace around it (a credential pasted with a line break still has it) and
  /// regardless of case. A value shorter than [`Self::SHORT`] characters counts only as a whole
  /// token, between characters that are not letters or digits, so `abc` is not found in `abcde`.
  pub fn check(&self, content: MemoryContent) -> Result<ScreenedContent, ContainsSecret> {
    let fields = [
      content.text().to_lowercase(),
      content
        .key()
        .map_or_else(String::new, |key| key.as_str().to_lowercase()),
      content.tags().joined(),
    ];
    for (name, value) in &self.values {
      let value = Zeroizing::new(value.expose().trim().to_lowercase());
      let whole_token = value.chars().count() < Self::SHORT;
      if fields.iter().any(|field| holds(field, &value, whole_token)) {
        return Err(ContainsSecret { secret: name.clone() });
      }
    }
    Ok(ScreenedContent(content))
  }
}

/// Whether `field` holds `value`, as a whole token only when `whole_token` says so.
fn holds(field: &str, value: &str, whole_token: bool) -> bool {
  if !whole_token {
    return field.contains(value);
  }
  field.match_indices(value).any(|(start, _)| {
    let before = field[..start].chars().next_back();
    let after = field[start + value.len()..].chars().next();
    !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
  })
}

impl std::fmt::Debug for SecretScreen {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let names: Vec<&SecretName> = self.values.iter().map(|(name, _)| name).collect();
    f.debug_struct("SecretScreen").field("secrets", &names).finish()
  }
}
