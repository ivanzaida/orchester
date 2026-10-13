//! The app's MCP server: agents look at and drive the running window the way browser-use drives a
//! browser, through one server owned by the app. It hosts lurq's window tools (`lurq_*`: screenshots,
//! the element tree, clicking and typing, logs) in-process (`McpConfig::hosted`) beside the app's own
//! (`tools`: `app_state`, `navigate`, `set_theme`, `hold_reads`), which lurq's registry serves and which run on the
//! event-loop thread with the window's state, and Orchester's domain tools (`domain`), which work on project
//! data off the event loop. What agents may do is [`McpSettings`].
//!
//! It listens on `127.0.0.1` only and requires a bearer token and a loopback `Host` on every request (`server`,
//! `guard`). The installed app listens on the fixed port set in Settings › Automation and keeps its token in the OS
//! secret store (`token`), so an agent registered once keeps connecting (`docs/design/app-mcp-connection.md`); a QA
//! copy, a test or a development build takes a free port and a new token on each start and writes both to its
//! discovery file ([`McpMode`], `discovery`). [`AppMcp`] runs it and publishes [`McpStatus`], which names the
//! address and where the token is kept, never the token; only a person's click in Settings shows or copies it.

mod clients;
pub mod connect;
mod control;
mod discovery;
pub mod domain;
mod guard;
pub mod input_gate;
mod link;
mod mode;
mod server;
pub mod settings;
mod status;
mod token;
pub mod tools;
mod view;

use lurq::{app::i18n::I18n, mcp::McpConfig};

pub use self::{
  control::{AppMcp, ControlError},
  discovery::DiscoveryFolder,
  input_gate::InputGate,
  link::WindowLink,
  mode::{DISCOVERY_VARIABLE, McpMode},
  server::{address, endpoint},
  settings::{McpSettings, WindowControlSource},
  status::{McpKind, McpStatus, SeenClient, ServerState},
  token::BearerToken,
  view::McpLink,
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
  add_project, connections (add_cli_connection, check_connection, list_models, add_mcp_server,   update_mcp_server, set_mcp_server_enabled, remove_mcp_server), agent profiles \
  (save_profile_draft, publish_profile), pipelines (create_pipeline, save_pipeline_draft, publish_pipeline), \
  instructions, project settings, then tasks (create_task, run_task), runs (get_run, send_message), \
  approvals, questions, plans, get_changes and merge_run. Name a project by its id or name, a task by its \
  key (KON-12), a run by its id or number. A refusal is a tool error with a stable `code` and the app's own \
  explanation in `reasons`. Answering approvals works only while a person turned on Let automation answer \
  approvals in the app; widening what agents may do (broader profile permissions or MCP servers, a CLI \
  program not found on the search path, environment variables that change program loading, adding or widening   an MCP server or turning one on) is refused as \
  person_only unless a person turned on Let automation change agent permissions in Settings › What automation may do. \
  Narrowing is always allowed, and no tool takes or returns a secret's value.";

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
