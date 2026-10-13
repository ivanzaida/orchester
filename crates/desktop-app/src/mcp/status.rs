//! What the window shows of the app's MCP server: whether it listens and where, why not, how it was started,
//! whether agents may control the window, and who connected. It names the address and where the token is kept,
//! never the token.

use std::{num::NonZeroU16, path::PathBuf, time::SystemTime};

use orchester_core::app_mcp::McpPort;
use orchester_workspace::messages::Message;

use super::settings::McpSettings;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpStatus {
  pub server: ServerState,
  pub kind: McpKind,
  /// The installed app's port as its settings hold it, whether or not the server listens there; `None` before
  /// they were read, and for a copy that takes a new port on each start.
  pub port: Option<McpPort>,
  /// What agents may do with the window, which the status bar shows while they may control it.
  pub settings: McpSettings,
  /// The clients seen since the app started, or since the token changed.
  pub clients: Vec<SeenClient>,
  /// Whether the token changed while the app runs: the clients are those since then.
  pub rotated: bool,
}

impl lurq::app::component::DevtoolsInspectable for McpStatus {}

impl McpStatus {
  /// A server that never started, e.g. for a window without one.
  pub fn unavailable(kind: McpKind, settings: McpSettings) -> Self {
    Self {
      server: ServerState::Unavailable,
      kind,
      port: None,
      settings,
      clients: Vec::new(),
      rotated: false,
    }
  }

  /// The port the server listens on, while it listens.
  pub const fn listening_port(&self) -> Option<NonZeroU16> {
    match self.server {
      ServerState::Listening { port } => Some(port),
      _ => None,
    }
  }

  /// Whether agents may control the window now: only while the server listens.
  pub const fn window_control(&self) -> bool {
    self.listening_port().is_some() && self.settings.window_control()
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerState {
  Listening {
    port: NonZeroU16,
  },
  /// Another program holds the installed app's port, so it does not listen; it never moves to another port.
  PortTaken {
    port: McpPort,
  },
  /// The person turned the server off in Settings.
  Off,
  /// The token cannot be read from the secret store, or a first one cannot be stored there, and why.
  TokenUnreadable {
    reason: Message,
  },
  /// It did not start for another reason; the log says why.
  Unavailable,
}

/// How the app was started, which decides the address and where the token is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpKind {
  /// The installed app: the address set in Settings, the token in the secret store.
  Installation,
  /// A QA copy or a test harness: a new port and token on each start, its discovery file in `folder`.
  Qa { folder: PathBuf },
  /// A development build: a new port and token on each start, its discovery file in the user's folder.
  Development { file: Option<PathBuf> },
}

/// A client as it named itself, and when it last asked something.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeenClient {
  pub name: String,
  pub version: String,
  /// Whether one of its sessions is open.
  pub connected: bool,
  pub last_request: SystemTime,
}
