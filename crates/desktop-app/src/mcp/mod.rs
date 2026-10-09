//! The app's MCP server: agents look at and drive the running window the way browser-use drives a
//! browser, through one server owned by the app. It hosts lurq's window tools (`lurq_*`: screenshots,
//! the element tree, clicking and typing, logs) in-process (`McpConfig::hosted`) beside the app's own
//! (`tools`: `app_state`, `navigate`, `set_theme`, `hold_reads`), which lurq's registry serves and which run on the
//! event-loop thread with the window's state, and Orchester's domain tools (`domain`), which work on project
//! data off the event loop. What agents may do is [`McpSettings`].
//!
//! It listens on `127.0.0.1` on a port the OS picks and requires its own bearer token on every request
//! (`server`, `guard`). The port and the token are written to this process's discovery file in a folder
//! of the app's own ([`DiscoveryFolder`]), which only the user can read and which is removed when the
//! server stops; files of processes that ended without removing theirs are removed when the next
//! server starts. [`McpStatus`] carries the port only, so nothing in the window can show, copy or log
//! the token.

mod discovery;
pub mod domain;
mod guard;
mod link;
mod server;
pub mod settings;
pub mod tools;

use std::{num::NonZeroU16, path::Path};

use lurq::{
  app::{Tree, i18n::I18n},
  mcp::McpConfig,
};
use tokio::runtime::Handle;

use self::{discovery::DiscoveryFile, server::Listening};
pub use self::{
  discovery::DiscoveryFolder,
  link::WindowLink,
  server::{BearerToken, endpoint},
  settings::{McpSettings, WindowControlSource},
};
use crate::{appearance::Appearance, services::Services};

/// The name in the discovery file and the server's info, apart from the deprecated desktop's
/// `orchester`.
pub const APP_NAME: &str = "orchester-app";

const INSTRUCTIONS: &str = "Orchester is a desktop app that orchestrates coding agents per project. `app_state` \
  tells where its window is (route, tabs, theme, size); `navigate` takes the active tab to a route by name; \
  `set_theme` switches dark, light or system colors and keeps the choice; debug builds add `hold_reads`, which \
  holds the reads of project data so a loading state can be captured. The `lurq_*` tools look at and drive the \
  window itself. Changing the window (navigate, set_theme, lurq input) needs window control, which a \
  release build grants only when the app's settings allow it or ORCHESTER_APP_MCP_WINDOW_CONTROL=on. \
  The domain tools set a project up and drive its work through data, with the rules of the app's screens: \
  add_project, connections (add_cli_connection, check_connection, list_models), agent profiles \
  (save_profile_draft, publish_profile), pipelines (create_pipeline, save_pipeline_draft, publish_pipeline), \
  instructions, project settings, then tasks (create_task, run_task), runs (get_run, send_message), \
  approvals, questions, plans, get_changes and merge_run. Name a project by its id or name, a task by its \
  key (KON-12), a run by its id or number. A refusal is a tool error with a stable `code` and the app's own \
  explanation in `reasons`. Answering approvals works only while a person turned on Let automation answer \
  approvals in the app; widening what agents may do (broader profile permissions or MCP servers, a CLI \
  program not found on the search path, environment variables that change program loading) is refused as \
  person_only unless a person turned on Let automation change agent permissions in Settings › Automation. \
  Narrowing is always allowed, and no tool takes or returns a secret's value.";

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

/// lurq's tools for `settings`, the app's tools on `link` and `appearance`, and, for a started session,
/// the domain tools on its `services` (refusals worded by `i18n`) with `hold_reads` in debug builds.
pub fn config(
  settings: McpSettings,
  link: &WindowLink,
  appearance: &Appearance,
  session: Option<&Services>,
  i18n: &I18n,
) -> McpConfig {
  let hold = session.map(|services| services.hold.clone());
  let domain = session.map(|services| domain::tools(services, i18n.clone()));
  tools::all(link, appearance, settings, hold)
    .into_iter()
    .chain(domain.into_iter().flatten())
    .fold(
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
