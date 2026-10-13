//! What the desktop gives the runner for a turn of the project's Orchestrator chat: where the agent reaches
//! Orchester's domain tools and the token it presents there.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The endpoint of the chat's domain tools on the desktop (`http://127.0.0.1:<port>/mcp`) and its bearer token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatTools {
  pub url: String,
  pub token: ChatToken,
}

/// The bearer token of a chat's domain tools. It is a secret: it shows in no `Debug` output and has no `Display`;
/// the runner writes it only into the agent's MCP configuration file.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChatToken(String);

impl ChatToken {
  pub const fn new(token: String) -> Self {
    Self(token)
  }

  pub fn expose(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for ChatToken {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("ChatToken(<redacted>)")
  }
}
