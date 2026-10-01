//! Tools of an MCP server that run without asking (`docs/interactive-tool-approvals.md`, "Tools that
//! run without asking"). A profile that asks before MCP actions sends every call of its servers' tools
//! to a person; a server can name tools that do not ask, typically the ones that only read. The list
//! changes nothing for a profile that does not ask: its agents' calls are allowed or declined as
//! before.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::McpServerName;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum McpToolNameError {
  #[error("an MCP tool needs a name")]
  Blank,
  #[error("an MCP tool name is at most {max} characters long", max = McpToolName::MAX_CHARS)]
  TooLong,
  #[error("an MCP tool name has only letters, digits, `-` and `_`: {0}")]
  Invalid(String),
}

/// What a server calls one of its tools, e.g. `get_screenshot`. ASCII letters, digits, `-` and `_`,
/// at most [`Self::MAX_CHARS`]: the names both CLIs pass on unchanged (Claude Code as
/// `mcp__<server>__<tool>`, Codex as the tool of `mcp_servers.<server>`). A name with other characters
/// is renamed by the CLI, so a rule naming it as the server writes it would never match.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McpToolName(String);

impl McpToolName {
  pub const MAX_CHARS: usize = 64;

  /// Surrounding whitespace is not part of the name.
  pub fn new(value: &str) -> Result<Self, McpToolNameError> {
    let name = value.trim();
    if name.is_empty() {
      return Err(McpToolNameError::Blank);
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
      return Err(McpToolNameError::Invalid(name.to_owned()));
    }
    if name.len() > Self::MAX_CHARS {
      return Err(McpToolNameError::TooLong);
    }
    Ok(Self(name.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for McpToolName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for McpToolName {
  type Error = McpToolNameError;

  fn try_from(value: String) -> Result<Self, McpToolNameError> {
    Self::new(&value)
  }
}

impl From<McpToolName> for String {
  fn from(name: McpToolName) -> Self {
    name.0
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ExemptToolsError {
  #[error("{0}")]
  Name(#[from] McpToolNameError),
  #[error("the tool {0} is listed twice")]
  Duplicate(String),
  #[error("at most {max} tools of a server run without asking", max = ExemptTools::MAX)]
  TooMany,
}

/// The tools of one server that run without asking when a profile asks before MCP actions, in the
/// order they were listed. None by default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Vec<McpToolName>", into = "Vec<McpToolName>")]
pub struct ExemptTools(Vec<McpToolName>);

impl ExemptTools {
  pub const MAX: usize = 64;

  /// No tools: every call asks.
  pub const fn none() -> Self {
    Self(Vec::new())
  }

  pub fn new(tools: Vec<McpToolName>) -> Result<Self, ExemptToolsError> {
    if tools.len() > Self::MAX {
      return Err(ExemptToolsError::TooMany);
    }
    for (index, tool) in tools.iter().enumerate() {
      if tools[..index].contains(tool) {
        return Err(ExemptToolsError::Duplicate(tool.to_string()));
      }
    }
    Ok(Self(tools))
  }

  /// One tool name per line, as the server's dialog takes them; empty lines are left out.
  pub fn parse(text: &str) -> Result<Self, ExemptToolsError> {
    let tools = text
      .lines()
      .filter(|line| !line.trim().is_empty())
      .map(McpToolName::new)
      .collect::<Result<Vec<_>, _>>()?;
    Self::new(tools)
  }

  pub fn tools(&self) -> &[McpToolName] {
    &self.0
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn len(&self) -> usize {
    self.0.len()
  }

  pub fn contains(&self, tool: &str) -> bool {
    self.0.iter().any(|listed| listed.as_str() == tool)
  }
}

impl TryFrom<Vec<McpToolName>> for ExemptTools {
  type Error = ExemptToolsError;

  fn try_from(tools: Vec<McpToolName>) -> Result<Self, ExemptToolsError> {
    Self::new(tools)
  }
}

impl From<ExemptTools> for Vec<McpToolName> {
  fn from(tools: ExemptTools) -> Self {
    tools.0
  }
}

/// The tools that run without asking in one invocation, by server: those of the servers it gives its
/// agent. It decides only for an invocation that asks before MCP actions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvocationExemptions(Vec<(McpServerName, ExemptTools)>);

impl InvocationExemptions {
  /// The servers with tools that run without asking; a server without any is left out.
  pub fn new(servers: impl IntoIterator<Item = (McpServerName, ExemptTools)>) -> Self {
    Self(servers.into_iter().filter(|(_, tools)| !tools.is_empty()).collect())
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn servers(&self) -> impl Iterator<Item = (&McpServerName, &ExemptTools)> {
    self.0.iter().map(|(server, tools)| (server, tools))
  }

  /// Whether `tool` of `server` runs without asking, as Codex names a call (`server`, `tool`).
  pub fn exempts(&self, server: &str, tool: &str) -> bool {
    self
      .0
      .iter()
      .any(|(name, tools)| name.as_str() == server && tools.contains(tool))
  }

  /// The server and tool of a Claude Code tool named `mcp__<server>__<tool>` that runs without
  /// asking; `None` for any other tool.
  pub fn claude_tool(&self, name: &str) -> Option<(&McpServerName, &McpToolName)> {
    self
      .0
      .iter()
      .flat_map(|(server, tools)| tools.tools().iter().map(move |tool| (server, tool)))
      .find(|(server, tool)| claude_tool_name(server, tool) == name)
  }

  /// One Claude Code allow rule per tool: `mcp__<server>__<tool>`.
  pub fn claude_rules(&self) -> impl Iterator<Item = String> {
    self
      .0
      .iter()
      .flat_map(|(server, tools)| tools.tools().iter().map(move |tool| claude_tool_name(server, tool)))
  }
}

/// How Claude Code names `tool` of `server`, and its permission rule for exactly that tool.
fn claude_tool_name(server: &McpServerName, tool: &McpToolName) -> String {
  format!("mcp__{server}__{tool}")
}
