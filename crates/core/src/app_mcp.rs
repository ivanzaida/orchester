//! The fixed address of `orchester-app`'s own MCP server in an installed build (`docs/design/app-mcp-connection.md`):
//! the port the person sets in Settings › Automation. The server listens on loopback only; the port is all that is
//! chosen.

use thiserror::Error;

/// A port below 1024 needs privileges on macOS and is reserved for system services, and port 0 would let the OS
/// pick one, which a fixed address must never do.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("a port from 1024 to 65535 is needed")]
pub struct InvalidPort;

/// The port the installed app's MCP server listens on, 1024–65535.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct McpPort(u16);

impl McpPort {
  /// `47821`: unassigned by IANA and outside the common development servers' ranges (decided 2026-10-13).
  pub const DEFAULT: Self = Self(47821);

  pub fn new(port: u32) -> Result<Self, InvalidPort> {
    match u16::try_from(port) {
      Ok(port @ 1024..=u16::MAX) => Ok(Self(port)),
      _ => Err(InvalidPort),
    }
  }

  /// A typed port: whole digits only, spaces around them ignored.
  pub fn parse(text: &str) -> Result<Self, InvalidPort> {
    let text = text.trim();
    if text.is_empty() || text.len() > 5 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
      return Err(InvalidPort);
    }
    text.parse::<u32>().map_err(|_| InvalidPort).and_then(Self::new)
  }

  pub const fn get(self) -> u16 {
    self.0
  }
}

impl Default for McpPort {
  fn default() -> Self {
    Self::DEFAULT
  }
}
