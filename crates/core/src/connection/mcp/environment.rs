//! The variables a stdio MCP server starts with: plain text, or the value of a project secret, read
//! from the keychain when the agent starts and never written anywhere else (see
//! [`crate::connection::invocation::mcp`]).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{id::SecretId, resource::VariableName};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum McpEnvironmentError {
  /// Windows does not tell names apart by case, so neither does a server.
  #[error("the environment variable `{0}` is set twice")]
  Duplicate(String),
  #[error("the value of `{0}` contains a NUL character")]
  Nul(String),
}

/// The value of one variable.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum McpEnvValue {
  /// Stored and shown as it is, so it holds no secret.
  Text(String),
  /// The value of this project secret.
  Secret(SecretId),
}

/// Variables of a stdio server, with distinct names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
  try_from = "BTreeMap<VariableName, McpEnvValue>",
  into = "BTreeMap<VariableName, McpEnvValue>"
)]
pub struct McpEnvironment(BTreeMap<VariableName, McpEnvValue>);

impl McpEnvironment {
  pub fn new(variables: BTreeMap<VariableName, McpEnvValue>) -> Result<Self, McpEnvironmentError> {
    let mut seen = BTreeSet::new();
    for (name, value) in &variables {
      if !seen.insert(name.as_str().to_ascii_uppercase()) {
        return Err(McpEnvironmentError::Duplicate(name.as_str().to_owned()));
      }
      if matches!(value, McpEnvValue::Text(text) if text.contains('\0')) {
        return Err(McpEnvironmentError::Nul(name.as_str().to_owned()));
      }
    }
    Ok(Self(variables))
  }

  pub fn iter(&self) -> impl Iterator<Item = (&VariableName, &McpEnvValue)> {
    self.0.iter()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn len(&self) -> usize {
    self.0.len()
  }

  /// The secrets the variables take their values from, each once.
  pub fn secrets(&self) -> impl Iterator<Item = SecretId> + '_ {
    let mut seen = BTreeSet::new();
    self.0.values().filter_map(move |value| match value {
      McpEnvValue::Secret(secret) if seen.insert(*secret) => Some(*secret),
      _ => None,
    })
  }
}

impl TryFrom<BTreeMap<VariableName, McpEnvValue>> for McpEnvironment {
  type Error = McpEnvironmentError;

  fn try_from(variables: BTreeMap<VariableName, McpEnvValue>) -> Result<Self, McpEnvironmentError> {
    Self::new(variables)
  }
}

impl From<McpEnvironment> for BTreeMap<VariableName, McpEnvValue> {
  fn from(environment: McpEnvironment) -> Self {
    environment.0
  }
}
