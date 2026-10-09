//! The UI kit's gallery for design QA: every control of `orchester_app::ui` in its states, in a 1440 ×
//! 900 window with the app's theme, locales and MCP server, so an agent can look at it and drive it
//! like the app (`lurq_screenshot`, `lurq_read_tree`, `lurq_interact`). It is a development example and
//! never part of the app.
//!
//! Run it with `cargo run -p orchester-app --example ui_gallery`. Its console log names its pid and the
//! discovery file of its MCP server, `%LOCALAPPDATA%\orchester-app-mcp\<pid>.json` (on macOS under
//! `~/Library/Application Support`), which holds the port and the bearer token. Page tabs at its top
//! switch the sections; Dark and Light switch the theme. The sample words are fixture text, not the
//! app's.

mod gallery;
mod sections;

use lurq::{
  app::{App, Tree, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow},
  mcp::McpConfig,
};
use orchester_app::{
  app_log, i18n,
  mcp::{APP_NAME, AppMcp, DiscoveryFolder, InputGate, McpSettings},
  theme::{self, Mode},
  ui::loader,
};
use tokio::runtime::Runtime;

use self::gallery::Gallery;

/// `1` opens the window without taking the focus, as the app does for agents.
const START_WITHOUT_FOCUS_VARIABLE: &str = "ORCHESTER_APP_MCP_START_WITHOUT_FOCUS";

fn main() {
  app_log::install();
  let runtime = Runtime::new().expect("the OS provides threads for the MCP server's runtime");
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app).expect("the bundled locale files are valid JSON; covered by tests/i18n.rs");

  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  loader::install(&mut tree);
  let settings = McpSettings::of_build().with_environment();
  let config = McpConfig::new().app_name(APP_NAME).scopes(settings.scopes());
  // The gallery shows no person-only control, so nothing is protected.
  let server = AppMcp::start(
    &mut tree,
    runtime.handle(),
    &DiscoveryFolder::User,
    config,
    &InputGate::default(),
  );
  match &server {
    Some(server) => tracing::info!(
      pid = std::process::id(),
      discovery = %server.discovery_path().display(),
      "the UI kit's gallery is open"
    ),
    None => tracing::warn!(
      pid = std::process::id(),
      "the UI kit's gallery runs without its MCP server"
    ),
  }
  tree.mount_root::<Gallery>(&mut app, ());

  let without_focus = std::env::var_os(START_WITHOUT_FOCUS_VARIABLE).is_some_and(|value| value == "1");
  WinitWindow::new(app, tree)
    .with_title("Orchester UI kit")
    .with_size(1440, 900)
    .with_start_without_focus(without_focus)
    .run();
  drop(server);
}
