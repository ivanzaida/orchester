use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum McpServerNameError {
  #[error("an MCP server needs a name")]
  Blank,
  #[error("an MCP server name is at most {max} characters long", max = McpServerName::MAX_CHARS)]
  TooLong,
  #[error("an MCP server name starts with a small letter and has only small letters, digits, `-` and `_`")]
  Invalid,
}

/// What an MCP server is called, e.g. `project-knowledge`. The agent CLIs use it as the server's key:
/// Codex in its configuration (`mcp_servers.<name>`), Claude Code in the names of the server's tools
/// (`mcp__<name>__<tool>`) and in its permission rules. Small ASCII letters, digits, `-` and `_`
/// are what both accept unchanged; the first character is a letter. Unique within the project.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McpServerName(String);

impl McpServerName {
  pub const MAX_CHARS: usize = 48;

  /// Surrounding whitespace is not part of the name.
  pub fn new(value: &str) -> Result<Self, McpServerNameError> {
    let name = value.trim();
    if name.is_empty() {
      return Err(McpServerNameError::Blank);
    }
    if name.chars().count() > Self::MAX_CHARS {
      return Err(McpServerNameError::TooLong);
    }
    let mut chars = name.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    let rest = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !(first && rest) {
      return Err(McpServerNameError::Invalid);
    }
    Ok(Self(name.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for McpServerName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for McpServerName {
  type Error = McpServerNameError;

  fn try_from(value: String) -> Result<Self, McpServerNameError> {
    Self::new(&value)
  }
}

impl From<McpServerName> for String {
  fn from(name: McpServerName) -> Self {
    name.0
  }
}
