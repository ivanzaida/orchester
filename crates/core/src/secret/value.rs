use std::fmt;

use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecretValueError {
  /// Empty, or only whitespace. The rejected text is not part of the error.
  #[error("a secret value must not be blank")]
  Blank,
}

/// A secret's value in memory, on its way to the keychain. `Debug` shows no content, and there is
/// no `Display`, `Clone`, comparison or serialization, so the value cannot reach a log, a stored
/// row, an error or a copy by accident. Its memory is overwritten when it is dropped. The value is
/// read only through [`SecretValue::expose`], which the keychain boundary calls.
pub struct SecretValue(String);

impl SecretValue {
  /// Takes `value` without copying it. A blank value is refused and wiped. The value is kept as
  /// entered: whitespace around it may be part of a credential.
  pub fn new(mut value: String) -> Result<Self, SecretValueError> {
    if value.trim().is_empty() {
      value.zeroize();
      return Err(SecretValueError::Blank);
    }
    Ok(Self(value))
  }

  /// The value itself, for the one place that hands it to the OS keychain. Nothing else reads it:
  /// secret values never reach prompts, variables, logs or the screen.
  pub fn expose(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for SecretValue {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("SecretValue(<redacted>)")
  }
}

impl Drop for SecretValue {
  fn drop(&mut self) {
    self.0.zeroize();
  }
}

impl ZeroizeOnDrop for SecretValue {}
