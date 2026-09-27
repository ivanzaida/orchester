//! Adding a secret, replacing its value and archiving it. The keychain and the project database
//! change one after the other, in the order that keeps them consistent when the second change
//! fails: a secret the database lists always has its value in the keychain, and a value whose
//! secret could not be stored is removed again.

use thiserror::Error;

use super::{Environment, NewSecret, Secret, SecretName, SecretValue, SecretVault, VaultError};
use crate::{
  id::{ProjectId, SecretId},
  store::SecretStore,
  time::Timestamp,
};

#[derive(Debug, Error)]
pub enum SecretError<E: std::error::Error + 'static> {
  /// Another secret of the project already has this name in this environment.
  #[error("a secret named {name} already exists in {environment}", environment = .environment.as_str())]
  Taken { name: SecretName, environment: Environment },
  #[error("secret {0} does not exist")]
  NotFound(SecretId),
  #[error("secret {0} is archived")]
  Archived(SecretId),
  #[error("keychain: {0}")]
  Vault(#[source] VaultError),
  #[error("project database: {0}")]
  Store(#[source] E),
  /// The secret could not be stored, and its value could not be removed from the keychain again.
  #[error("project database: {source}; the value could not be removed from the keychain again: {cleanup}")]
  ValueLeftBehind { source: E, cleanup: VaultError },
  /// The keychain holds the new value, but the database does not record the rotation.
  #[error("the new value of secret {secret} is stored, but its rotation is not recorded: {source}")]
  RotationNotRecorded { secret: SecretId, source: E },
}

/// The active secret among `secrets` that has `name` in `environment`, if there is one: a new
/// secret cannot take its name there (07.A `m9Dhp`).
pub fn existing<'a>(secrets: &'a [Secret], name: &SecretName, environment: Environment) -> Option<&'a Secret> {
  secrets
    .iter()
    .find(|secret| !secret.is_archived() && secret.name() == name && secret.environment() == environment)
}

/// Adds a secret to `project`: its value goes into the keychain first, then its metadata into the
/// project database. When the database refuses the secret, the value is removed again.
pub fn add<S, V>(
  store: &mut S,
  vault: &V,
  project: ProjectId,
  new: NewSecret,
  value: &SecretValue,
  now: Timestamp,
) -> Result<Secret, SecretError<S::Error>>
where
  S: SecretStore + ?Sized,
  V: SecretVault + ?Sized,
{
  let secrets = store.secrets().map_err(SecretError::Store)?;
  if existing(&secrets, &new.name, new.environment).is_some() {
    return Err(SecretError::Taken {
      name: new.name,
      environment: new.environment,
    });
  }
  let secret = Secret::new(SecretId::generate(), new, now);
  let key = secret.vault_key(project);
  vault.store(&key, value).map_err(SecretError::Vault)?;
  if let Err(source) = store.create_secret(&secret) {
    return Err(match vault.delete(&key) {
      Ok(()) => SecretError::Store(source),
      Err(cleanup) => SecretError::ValueLeftBehind { source, cleanup },
    });
  }
  Ok(secret)
}

/// Replaces the value of secret `id` (rotation). The keychain keeps one value per secret, so the
/// previous one is gone once the new one is stored; tools use the new value from their next call.
pub fn replace_value<S, V>(
  store: &mut S,
  vault: &V,
  project: ProjectId,
  id: SecretId,
  value: &SecretValue,
  now: Timestamp,
) -> Result<Secret, SecretError<S::Error>>
where
  S: SecretStore + ?Sized,
  V: SecretVault + ?Sized,
{
  let mut secret = active(store, id)?;
  vault
    .store(&secret.vault_key(project), value)
    .map_err(SecretError::Vault)?;
  secret.record_rotation(now);
  store
    .save_secret(&secret)
    .map_err(|source| SecretError::RotationNotRecorded { secret: id, source })?;
  Ok(secret)
}

/// Archives secret `id`: its value is removed from the keychain, then the secret is marked
/// archived. It leaves the project's list and can no longer be used; tools that refer to it are
/// no longer ready. Archiving an archived secret changes nothing, so a failed attempt can be
/// repeated.
pub fn archive<S, V>(
  store: &mut S,
  vault: &V,
  project: ProjectId,
  id: SecretId,
  now: Timestamp,
) -> Result<Secret, SecretError<S::Error>>
where
  S: SecretStore + ?Sized,
  V: SecretVault + ?Sized,
{
  let mut secret = store
    .secret(id)
    .map_err(SecretError::Store)?
    .ok_or(SecretError::NotFound(id))?;
  if secret.is_archived() {
    return Ok(secret);
  }
  vault.delete(&secret.vault_key(project)).map_err(SecretError::Vault)?;
  secret.archive(now);
  store.save_secret(&secret).map_err(SecretError::Store)?;
  Ok(secret)
}

fn active<S: SecretStore + ?Sized>(store: &S, id: SecretId) -> Result<Secret, SecretError<S::Error>> {
  let secret = store
    .secret(id)
    .map_err(SecretError::Store)?
    .ok_or(SecretError::NotFound(id))?;
  if secret.is_archived() {
    return Err(SecretError::Archived(id));
  }
  Ok(secret)
}
