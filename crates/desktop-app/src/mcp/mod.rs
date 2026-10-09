//! The app's MCP server: agents look at and drive the running window the way browser-use drives a
//! browser, through one server owned by the app. It hosts lurq's window tools (`lurq_*`: screenshots,
//! the element tree, clicking and typing, logs) in-process (`McpConfig::hosted`) beside the app's own
//! (`tools`: `app_state`, `navigate`, `set_theme`, `hold_reads`), which lurq's registry serves and which run on the
//! event-loop thread with the window's state. What agents may do is [`McpSettings`].
//!
//! It listens on `127.0.0.1` on a port the OS picks and requires its own bearer token on every request
//! (`server`, `guard`). The port and the token are written to this process's discovery file in a folder
//! of the app's own ([`DiscoveryFolder`]), which only the user can read and which is removed when the
//! server stops; files of processes that ended without removing theirs are removed when the next
//! server starts. [`McpStatus`] carries the port only, so nothing in the window can show, copy or log
//! the token.

mod discovery;
mod guard;
mod link;
mod server;
pub mod settings;
pub mod tools;

use std::{num::NonZeroU16, path::Path};

use lurq::{app::Tree, mcp::McpConfig};
use tokio::runtime::Handle;

use self::{discovery::DiscoveryFile, server::Listening};
pub use self::{
  discovery::DiscoveryFolder,
  link::WindowLink,
  server::{BearerToken, endpoint},
  settings::{McpSettings, WindowControlSource},
};
use crate::{appearance::Appearance, project::hold::ReadHold};

/// The name in the discovery file and the server's info, apart from the deprecated desktop's
/// `orchester`.
pub const APP_NAME: &str = "orchester-app";

const INSTRUCTIONS: &str = "Orchester is a desktop app that orchestrates coding agents per project. `app_state` \
  tells where its window is (route, tabs, theme, size); `navigate` takes the active tab to a route by name; \
  `set_theme` switches dark, light or system colors and keeps the choice; debug builds add `hold_reads`, which \
  holds the reads of project data so a loading state can be captured. The `lurq_*` tools look at and drive the \
  window itself. Changing the window (navigate, set_theme, lurq input) needs window control, which a \
  release build grants only when the app's settings allow it or ORCHESTER_APP_MCP_WINDOW_CONTROL=on.";

/// Whether the server listens, decided once at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpStatus {
  Listening {
    port: NonZeroU16,
    /// Whether agents may control the window, which the status bar shows while they may.
    window_control: bool,
  },
  /// It did not start; the log says why.
  Unavailable,
}

impl lurq::app::component::DevtoolsInspectable for McpStatus {}

impl McpStatus {
  pub fn of(server: Option<&AppMcp>, settings: McpSettings) -> Self {
    server.map_or(Self::Unavailable, |server| Self::Listening {
      port: server.port(),
      window_control: settings.window_control(),
    })
  }
}

/// lurq's tools for `settings` and the app's tools on `link` and `appearance`, with `hold_reads` on
/// `hold` in debug builds of a started session.
pub fn config(settings: McpSettings, link: &WindowLink, appearance: &Appearance, hold: Option<ReadHold>) -> McpConfig {
  tools::all(link, appearance, settings, hold).into_iter().fold(
    McpConfig::new()
      .app_name(APP_NAME)
      .instructions(INSTRUCTIONS)
      .scopes(settings.scopes()),
    McpConfig::tool,
  )
}

/// The running server and its discovery file; dropping it stops the server, then removes the file.
pub struct AppMcp {
  listening: Listening,
  discovery: DiscoveryFile,
}

impl AppMcp {
  /// Hosts the tools of `config` on `tree`, the window's root tree, and serves them on `runtime`,
  /// writing the discovery file in `folder`. `None` when the server cannot listen or write its file,
  /// which the log says; the window runs without MCP then.
  pub fn start(tree: &mut Tree, runtime: &Handle, folder: &DiscoveryFolder, config: McpConfig) -> Option<Self> {
    let tools = tree.enable_mcp(config.hosted());
    let folder = folder.path()?;
    let token = BearerToken::generate();
    let listening = match Listening::start(runtime, tools, token.clone()) {
      Ok(listening) => listening,
      Err(error) => {
        tracing::error!(%error, "the app's MCP server did not start");
        return None;
      }
    };
    let discovery = match DiscoveryFile::write(&folder, listening.port(), token.expose()) {
      Ok(discovery) => discovery,
      Err(error) => {
        tracing::error!(%error, folder = %folder.display(), "the app's MCP server wrote no discovery file");
        return None;
      }
    };
    tracing::info!(
      endpoint = %endpoint(listening.port()),
      discovery = %discovery.path().display(),
      "the app's MCP server is listening"
    );
    Some(Self { listening, discovery })
  }

  pub const fn port(&self) -> NonZeroU16 {
    self.listening.port()
  }

  pub fn discovery_path(&self) -> &Path {
    self.discovery.path()
  }
}
