//! The app's MCP server: agents look at and drive the running window the way browser-use drives a
//! browser, through one server. It serves lurq's window tools (`lurq_*`: screenshots, the element tree,
//! clicking and typing, logs) and the app's own (`tools`: `app_state`, `navigate`, `set_theme`), which
//! run on the event-loop thread with the window's state. What agents may do is [`McpSettings`].
//!
//! It is lurq's embedded server, started and configured by the app: it listens on `127.0.0.1` on a port
//! the OS picks, requires a bearer token on every request and writes the token, with the port, to this
//! process's discovery file (`lurq/mcp/<pid>.json` in the user's local app data folder, the app named
//! `orchester-app`). The file is removed when the window closes; files of processes that ended without
//! removing theirs are removed when the next server starts. The token stays inside lurq: [`McpStatus`]
//! carries the port only, so nothing in the app can show, copy or log it.

mod link;
pub mod settings;
pub mod tools;

use std::num::NonZeroU16;

use lurq::{
  app::Tree,
  mcp::{McpConfig, McpHandle},
};

pub use self::{link::WindowLink, settings::McpSettings};
use crate::appearance::Appearance;

/// The name in the discovery file and the server's info, apart from the old desktop's `orchester`.
pub const APP_NAME: &str = "orchester-app";

const INSTRUCTIONS: &str = "Orchester is a desktop app that orchestrates coding agents per project. `app_state` \
  tells where its window is (route, tabs, theme, size); `navigate` takes the active tab to a route by name; \
  `set_theme` switches dark, light or system colors. The `lurq_*` tools look at and drive the window itself. \
  Changing the window (navigate, set_theme, lurq input) needs window control, which a release build grants \
  only when ORCHESTER_APP_MCP_WINDOW_CONTROL=on.";

/// Whether the server listens, decided once at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpStatus {
  Listening {
    port: NonZeroU16,
  },
  /// It did not start; lurq logs why.
  Unavailable,
}

impl lurq::app::component::DevtoolsInspectable for McpStatus {}

impl McpStatus {
  /// The status of a server lurq was asked to start: lurq returns no error when it cannot start one,
  /// it logs the cause and hands back a handle on port 0.
  pub fn of(handle: &McpHandle) -> Self {
    NonZeroU16::new(handle.port()).map_or(Self::Unavailable, |port| Self::Listening { port })
  }
}

/// The server's configuration: lurq's scopes for `settings` and the app's tools on `link` and
/// `appearance`.
pub fn config(settings: McpSettings, link: &WindowLink, appearance: &Appearance) -> McpConfig {
  tools::all(link, appearance).into_iter().fold(
    McpConfig::new()
      .app_name(APP_NAME)
      .instructions(INSTRUCTIONS)
      .scopes(settings.scopes()),
    McpConfig::tool,
  )
}

/// Starts the server on `tree`, the window's root tree.
pub fn start(tree: &mut Tree, settings: McpSettings, link: &WindowLink, appearance: &Appearance) -> McpStatus {
  let status = McpStatus::of(&tree.enable_mcp(config(settings, link, appearance)));
  match status {
    McpStatus::Listening { port } => tracing::info!(
      port,
      window_control = settings.window_control(),
      "MCP server listening on http://127.0.0.1:{port}/mcp"
    ),
    McpStatus::Unavailable => tracing::error!("the MCP server did not start; agents cannot reach this window"),
  }
  status
}
