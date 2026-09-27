use std::fmt;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecretNameError {
  #[error("a secret name must not be blank")]
  Blank,
  #[error("a secret name is {length} characters long; the limit is {}", SecretName::MAX_CHARS)]
  TooLong { length: usize },
  #[error("a secret name starts with a capital letter and has only capital letters, digits and underscores")]
  Invalid,
}

/// What a secret is called, e.g. `STAGING_DATABASE_URL`: a capital letter followed by capital
/// letters, digits and underscores, as the design writes every secret. A name is user data, not a
/// secret; it is shown and may be logged.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretName(String);

impl SecretName {
  pub const MAX_CHARS: usize = 128;

  /// Surrounding whitespace is not part of the name.
  pub fn new(value: &str) -> Result<Self, SecretNameError> {
    let trimmed = value.trim();
    let length = trimmed.chars().count();
    if length == 0 {
      return Err(SecretNameError::Blank);
    }
    if length > Self::MAX_CHARS {
      return Err(SecretNameError::TooLong { length });
    }
    let mut chars = trimmed.chars();
    let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_uppercase());
    let rest_valid = chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !(starts_with_letter && rest_valid) {
      return Err(SecretNameError::Invalid);
    }
    Ok(Self(trimmed.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for SecretName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}
