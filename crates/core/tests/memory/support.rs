//! Records, provenance and a keychain for the memory tests.

use std::collections::HashMap;

use orchester_core::{
  id::{ProjectId, ResourceId, RunId, StepExecutionId},
  memory::{AgentProvenance, MemoryContent, MemoryKind, ScreenedContent, SecretScreen},
  resource::StepKey,
  secret::{SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  time::Timestamp,
};
use parking_lot::Mutex;

pub const DAY: i64 = 86_400_000;

pub fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

pub fn content(kind: MemoryKind, text: &str) -> MemoryContent {
  MemoryContent::new(kind, text).expect("valid content")
}

/// `content` checked against no secrets.
pub fn screened(kind: MemoryKind, text: &str) -> ScreenedContent {
  SecretScreen::new(Vec::new())
    .check(content(kind, text))
    .expect("no secrets to hold")
}

pub fn provenance(profile: ResourceId) -> AgentProvenance {
  AgentProvenance {
    profile,
    run: RunId::generate(),
    step: StepKey::new("implement").expect("valid key"),
    execution: StepExecutionId::generate(),
  }
}

pub fn secret_value(text: &str) -> SecretValue {
  SecretValue::new(text.to_owned()).expect("not blank")
}

pub fn secret_name(name: &str) -> SecretName {
  SecretName::new(name).expect("valid name")
}

/// A keychain in memory that fails every read with `failure`, if set.
#[derive(Default)]
pub struct Keychain {
  pub values: Mutex<HashMap<VaultKey, String>>,
  pub failure: Option<VaultError>,
}

impl Keychain {
  pub fn with(project: ProjectId, secrets: &[(&orchester_core::secret::Secret, &str)]) -> Self {
    let values = secrets
      .iter()
      .map(|(secret, value)| (secret.vault_key(project), (*value).to_owned()))
      .collect();
    Self {
      values: Mutex::new(values),
      failure: None,
    }
  }
}

impl SecretVault for Keychain {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.values.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    if let Some(failure) = &self.failure {
      return Err(failure.clone());
    }
    let value = self.values.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.values.lock().remove(key);
    Ok(())
  }
}
