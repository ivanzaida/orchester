//! The closed sets a secret is filed under: what kind of credential it is, and the environment
//! it belongs to. Both are stored by their `as_str` identifiers.

use std::str::FromStr;

use thiserror::Error;

/// A stored identifier that names no kind or environment; the database was edited or damaged.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not a known value")]
pub struct UnknownValue(String);

/// What a secret is, as the Type select offers it (07.02 `uSpJF`). It decides the icon a secret
/// is shown with; it does not change how the value is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SecretKind {
  AccessToken,
  ApiKey,
  DatabaseConnection,
  DeploymentToken,
  Password,
}

impl SecretKind {
  pub const ALL: [Self; 5] = [
    Self::AccessToken,
    Self::ApiKey,
    Self::DatabaseConnection,
    Self::DeploymentToken,
    Self::Password,
  ];

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::AccessToken => "access_token",
      Self::ApiKey => "api_key",
      Self::DatabaseConnection => "database_connection",
      Self::DeploymentToken => "deployment_token",
      Self::Password => "password",
    }
  }
}

impl FromStr for SecretKind {
  type Err = UnknownValue;

  fn from_str(value: &str) -> Result<Self, UnknownValue> {
    Self::ALL
      .into_iter()
      .find(|kind| kind.as_str() == value)
      .ok_or_else(|| UnknownValue(value.to_owned()))
  }
}

/// The environment a secret is for (07.02 `fbIBM`). A name may be used once per environment, so
/// staging and production can each have their own `DEPLOY_TOKEN`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Environment {
  Development,
  Staging,
  Production,
}

impl Environment {
  pub const ALL: [Self; 3] = [Self::Development, Self::Staging, Self::Production];

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Development => "development",
      Self::Staging => "staging",
      Self::Production => "production",
    }
  }
}

impl FromStr for Environment {
  type Err = UnknownValue;

  fn from_str(value: &str) -> Result<Self, UnknownValue> {
    Self::ALL
      .into_iter()
      .find(|environment| environment.as_str() == value)
      .ok_or_else(|| UnknownValue(value.to_owned()))
  }
}
