//! What the Orchestrator chat proposes, or did itself, to the project's MCP servers (ORC-89, `docs/design/
//! orchestrator-toolset.md`, decision 11): adding one, changing its settings, turning it on or off, and removing one no
//! agent profile chooses, by the same rules as Connections › MCP servers. Adding or widening a server widens what
//! agents may do, so it is only ever a card a person applies; a change that only narrows runs at once while the
//! person lets automation change agent permissions. The card shows the server as applying leaves it, a secret by its
//! name only (`NAME@environment`), never a value; a variable by its name, never its value.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::id::ConnectionId;

/// An MCP server change of the chat's: what it does, to which server, the server as it shows it, and what applying it
/// checks again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpChange {
  pub operation: McpOperation,
  /// The server it changes; `None` for a new one.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub id: Option<ConnectionId>,
  /// The server as applying leaves it; for a removal, as it is removed.
  pub server: McpServerShown,
  /// It widens what agents may do: only a person applies it.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub widens: bool,
  /// A digest of the server as it was stored when it was proposed; empty for a new server. Applying is refused once
  /// the stored server differs.
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub basis: String,
  /// The chat's own arguments, carried out as they were given; a secret in them is a reference by name.
  #[serde(default, skip_serializing_if = "Map::is_empty")]
  pub arguments: Map<String, Value>,
}

/// What a change does to a server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpOperation {
  Add,
  Update,
  Enable,
  Disable,
  Remove,
}

impl McpOperation {
  /// The chat's tool that makes it (`mcp_add_server`, …).
  pub const fn tool(self) -> &'static str {
    match self {
      Self::Add => "mcp_add_server",
      Self::Update => "mcp_update_server",
      Self::Enable | Self::Disable => "mcp_set_server_enabled",
      Self::Remove => "mcp_remove_server",
    }
  }
}

/// A server as a card shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerShown {
  pub name: String,
  pub transport: McpTransportShown,
  /// The program's variables, by name.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub environment: Vec<McpVariableShown>,
  /// The server's tools that run without asking for profiles that ask before MCP actions; every other call asks.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub exempt_tools: Vec<String>,
  pub enabled: bool,
  /// The roles of the profiles that choose it.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub used_by: Vec<String>,
}

/// How a shown server is reached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpTransportShown {
  /// A program the agent's CLI starts: as written, the file it is found as on this app's search path (`None`: not
  /// found, which applying refuses), its arguments and the folder it starts in (`None`: the worktree).
  Stdio {
    program: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    found: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    working_directory: Option<String>,
  },
  /// A remote server: its URL, the values of its query left out, and what it is sent.
  Http {
    url: String,
    authorization: McpAuthorizationShown,
  },
}

/// What a remote server is sent, its secret by name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scheme", rename_all = "snake_case")]
pub enum McpAuthorizationShown {
  None,
  Bearer { secret: String },
  Basic { username: String, secret: String },
  BasicCredential { secret: String },
  Header { name: String, secret: String },
}

/// A variable of a server's program: its name, and the secret whose value it takes (`NAME@environment`), or `None`
/// for a plain value, which is not shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpVariableShown {
  pub name: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub secret: Option<String>,
}
