//! Storing, reading and removing a value through a keyring entry, the same on every platform that has a
//! store, and what keyring's errors mean for the vault.

use keyring_core::{Entry, Error};
use orchester_core::secret::{SecretValue, VaultError};
use zeroize::Zeroize;

/// Stores `value` in the credential `entry` names, creating or replacing it.
pub(crate) fn store(entry: keyring_core::Result<Entry>, value: &SecretValue) -> Result<(), VaultError> {
  entry
    .and_then(|entry| entry.set_secret(value.expose().as_bytes()))
    .map_err(vault_error)
}

/// The value of the credential `entry` names, stored as UTF-8 by [`store`].
pub(crate) fn read(entry: keyring_core::Result<Entry>) -> Result<SecretValue, VaultError> {
  let bytes = match entry.and_then(|entry| entry.get_secret()) {
    Ok(bytes) => bytes,
    Err(Error::NoEntry) => return Err(VaultError::NotFound),
    Err(error) => return Err(vault_error(error)),
  };
  let text = String::from_utf8(bytes).map_err(|error| {
    // The bytes may be a value; they are wiped, and the error does not carry them.
    let mut bytes = error.into_bytes();
    bytes.zeroize();
    VaultError::Failed("a stored value is malformed".to_owned())
  })?;
  SecretValue::new(text).map_err(|_| VaultError::Failed("a stored value is blank".to_owned()))
}

/// Removes the credential `entry` names; one that does not exist is already removed.
pub(crate) fn delete(entry: keyring_core::Result<Entry>) -> Result<(), VaultError> {
  match entry.and_then(|entry| entry.delete_credential()) {
    Ok(()) | Err(Error::NoEntry) => Ok(()),
    Err(error) => Err(vault_error(error)),
  }
}

/// keyring's error as a vault error. The variants that carry stored bytes (`BadEncoding`,
/// `BadDataFormat`) do not pass them on, and neither `Debug` output nor an unknown variant's text
/// is used: only messages that describe the failure are kept.
fn vault_error(error: Error) -> VaultError {
  match error {
    Error::NoStorageAccess(platform) => VaultError::AccessDenied(platform.to_string()),
    Error::TooLong(_, max) => VaultError::TooLong { max },
    Error::PlatformFailure(platform) => VaultError::Failed(platform.to_string()),
    Error::BadEncoding(_) | Error::BadDataFormat(..) => VaultError::Failed("a stored value is malformed".to_owned()),
    Error::BadStoreFormat(reason) | Error::NotSupportedByStore(reason) => VaultError::Failed(reason),
    Error::Invalid(attribute, reason) => VaultError::Failed(format!("{attribute} is invalid: {reason}")),
    Error::NoEntry => VaultError::Failed("the credential does not exist".to_owned()),
    Error::Ambiguous(entries) => VaultError::Failed(format!("{} credentials match", entries.len())),
    Error::NoDefaultStore => VaultError::Failed("no credential store is set up".to_owned()),
    // keyring may add variants; their text is not known to be free of stored bytes.
    _ => VaultError::Failed("the credential store failed".to_owned()),
  }
}
