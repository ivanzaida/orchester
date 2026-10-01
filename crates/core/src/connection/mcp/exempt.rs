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
///
/// Claude Code names a tool `mcp__<server>__<tool>`, and server names may hold `__` or end in `_`, so one
/// name can belong to two servers: `a` with `b__c` and `a__b` with `c`, `a` with `_x` and `a_` with `x`.
/// An allow rule for such a name would let the other server's tool run unasked too, so a listed tool
/// whose name another server of the invocation could also have gets no rule and still asks
/// ([`Self::claude_unapplied`]). Codex names a tool by its server and the tool apart, so it has none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvocationExemptions {
  exempt: Vec<(McpServerName, ExemptTools)>,
  /// Every server of the invocation, whose names a Claude Code tool name could belong to.
  servers: Vec<McpServerName>,
}

/// A listed tool Claude Code does not run without asking: its name could also be a tool of `other`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnappliedExemption {
  pub server: McpServerName,
  pub tool: McpToolName,
  pub other: McpServerName,
}

impl InvocationExemptions {
  /// The invocation's servers with the tools each runs without asking (none for most).
  pub fn new(servers: impl IntoIterator<Item = (McpServerName, ExemptTools)>) -> Self {
    let (mut exempt, mut names) = (Vec::new(), Vec::new());
    for (server, tools) in servers {
      names.push(server.clone());
      if !tools.is_empty() {
        exempt.push((server, tools));
      }
    }
    Self { exempt, servers: names }
  }

  pub fn is_empty(&self) -> bool {
    self.exempt.is_empty()
  }

  pub fn servers(&self) -> impl Iterator<Item = (&McpServerName, &ExemptTools)> {
    self.exempt.iter().map(|(server, tools)| (server, tools))
  }

  /// Whether `tool` of `server` runs without asking, as Codex names a call (`server`, `tool`).
  pub fn exempts(&self, server: &str, tool: &str) -> bool {
    self
      .exempt
      .iter()
      .any(|(name, tools)| name.as_str() == server && tools.contains(tool))
  }

  /// The server and tool of a Claude Code tool named `mcp__<server>__<tool>` that runs without
  /// asking; `None` for any other tool, and for a name another server could also have.
  pub fn claude_tool(&self, name: &str) -> Option<(&McpServerName, &McpToolName)> {
    self
      .listed()
      .filter(|(server, tool)| self.other_owner(server, tool).is_none())
      .find(|(server, tool)| claude_tool_name(server, tool) == name)
  }

  /// One Claude Code allow rule per tool, `mcp__<server>__<tool>`, but for names another server
  /// could also have.
  pub fn claude_rules(&self) -> Vec<String> {
    self
      .listed()
      .filter(|(server, tool)| self.other_owner(server, tool).is_none())
      .map(|(server, tool)| claude_tool_name(server, tool))
      .collect()
  }

  /// The listed tools that get no Claude Code rule, and the server their name could also belong to.
  pub fn claude_unapplied(&self) -> Vec<UnappliedExemption> {
    self
      .listed()
      .filter_map(|(server, tool)| {
        self.other_owner(server, tool).map(|other| UnappliedExemption {
          server: server.clone(),
          tool: tool.clone(),
          other: other.clone(),
        })
      })
      .collect()
  }

  fn listed(&self) -> impl Iterator<Item = (&McpServerName, &McpToolName)> {
    self
      .exempt
      .iter()
      .flat_map(|(server, tools)| tools.tools().iter().map(move |tool| (server, tool)))
  }

  /// Another server of the invocation whose tools Claude Code could name as `tool` of `server` is named.
  fn other_owner(&self, server: &McpServerName, tool: &McpToolName) -> Option<&McpServerName> {
    let name = claude_tool_name(server, tool);
    self.servers.iter().find(|other| {
      *other != server
        && name
          .strip_prefix(&format!("mcp__{other}__"))
          .is_some_and(|rest| !rest.is_empty())
    })
  }
}

/// How Claude Code names `tool` of `server`, and its permission rule for exactly that tool.
fn claude_tool_name(server: &McpServerName, tool: &McpToolName) -> String {
  format!("mcp__{server}__{tool}")
}
