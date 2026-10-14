// A release build is a window without a console: started from Explorer or the Start menu, a console
// program opens a console beside it. Debug builds keep the console and its log output.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use std::sync::{Arc, OnceLock};

use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_app::{
  app_log,
  appearance::{Appearance, ThemeSetting, system},
  brand, clipboard, i18n,
  mcp::{self, AppMcp, McpKind, McpLink, McpMode, McpSettings, McpStatus, WindowLink},
  root::{Root, RootProps},
  startup::{Launch, WindowPosition},
  system_icon, theme,
  ui::loader,
};
use orchester_store::StoreError;
use orchester_workspace::workspace::Workspace;
use tokio::runtime::Runtime;

/// For agents that drive the app over MCP while the person works in other windows: `1` opens the
/// window without taking the focus.
const START_WITHOUT_FOCUS_VARIABLE: &str = "ORCHESTER_APP_MCP_START_WITHOUT_FOCUS";

fn main() {
  let log = app_log::install();
  // Refused before anything opens in a QA copy or a debug build: a run that asked for a place must not get
  // a window elsewhere. The installed app ignores the variable (`WindowPosition::requested`).
  let position = WindowPosition::from_environment().unwrap_or_else(|error| {
    tracing::error!(%error, "the requested window position is refused; not starting");
    std::process::exit(2);
  });

  let runtime = Runtime::new().expect("the OS provides threads for the background runtime");
  // Started before the window exists: the data folder's lock, the session, the runners' client and
  // where the window opens, or why there is no session.
  let launch = Launch::begin(runtime.handle(), log);
  let workspace = launch.workspace();
  // Only this window moves a removed account's folder to the person's Recycle Bin / Trash; tests and QA copies have
  // a trash of their own.
  if let Some(workspace) = workspace {
    workspace.use_trash(std::sync::Arc::new(orchester_workspace::accounts::trash::SystemTrash));
  }
  let appearance = appearance(&runtime, workspace);
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app, appearance.mode());
  i18n::install(&app).expect("the bundled locale files are valid JSON; covered by tests/i18n.rs");

  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  loader::install(&mut tree);
  let link = WindowLink::default();
  let window_control = stored(workspace, Workspace::mcp_window_control, "window control").flatten();
  let mcp_settings = McpSettings::of_build().with_stored(window_control).with_environment();
  tracing::info!(
    window_control = mcp_settings.window_control(),
    source = mcp_settings.source().name(),
    "agents' control of the window"
  );
  // Started before the root mounts, so the status bar shows its address from the first frame.
  let server = McpMode::of_build(workspace).map(|mode| {
    let config = mcp::config(mcp_settings, &link, &appearance, launch.services(), app.i18n());
    AppMcp::start(
      &mut tree,
      runtime.handle(),
      mode,
      (config, mcp_settings),
      &link.input_gate(),
    )
  });
  let mcp = server.as_ref().map_or_else(
    // `McpMode::of_build` logged why there is none.
    || McpLink::without_server(McpStatus::unavailable(McpKind::Installation, mcp_settings)),
    McpLink::new,
  );
  let start = launch.start();
  tree.mount_root::<Root>(&mut app, RootProps { link, mcp, start });

  #[cfg(feature = "devtools")]
  {
    lurq::app::devtools::load_fonts(&mut app);
    tree.mount_devtools(&mut app);
  }

  let title = app.i18n().t("shell.app.name");
  let icon = brand::window_icon().expect("the bundled window icon is a valid PNG; covered by tests/brand.rs");
  let without_focus = std::env::var_os(START_WITHOUT_FOCUS_VARIABLE).is_some_and(|value| value == "1");
  let mut system_icon_pending = true;
  let window = WinitWindow::new(app, tree)
    .with_title(&title)
    .with_icon(icon)
    // The system's icons, once: the first frame is the first moment the UI thread has the window, which
    // is not shown yet unless it starts without the focus.
    .on_paint(move |_, _, _| {
      if std::mem::take(&mut system_icon_pending)
        && let Err(error) = system_icon::install()
      {
        tracing::warn!(%error, "the system may show a generic icon for the app");
      }
    })
    .with_start_without_focus(without_focus)
    // The window frames of the design (`Tasks · dark` `jn2IT`), title bar and status bar included.
    .with_size(1440, 900)
    .with_min_size(960, 640)
    // The app draws its own title bar and frame (`chrome`); the OS keeps the rounded corners.
    .with_decorations(false)
    .with_corner_radius(WindowCornerRadius::Rounded);
  // Part of the window's attributes, so it opens there rather than moving after it shows.
  match position {
    Some(WindowPosition { x, y }) => {
      tracing::info!(x, y, "the window opens at the requested physical position");
      window.with_position(x, y)
    }
    None => window,
  }
  .run();

  // `run` returns once the window has closed: the server stops and its discovery file goes, then the
  // runners stop and the session records its end.
  if let Some(server) = server {
    server.shutdown();
  }
  clipboard::clear_at_exit();
  launch.end();
}

/// The stored theme. The operating system's appearance is read on the runtime's blocking pool, never
/// on the UI thread; startup waits for it only when the stored theme is System, before the window
/// exists. Otherwise it arrives in the background, for a later choice of System.
fn appearance(runtime: &Runtime, workspace: Option<&Workspace>) -> Appearance {
  let theme = stored(workspace, Workspace::theme, "theme").unwrap_or_default();
  let system = Arc::new(OnceLock::new());
  let read = {
    let system = Arc::clone(&system);
    runtime.spawn_blocking(move || match system::mode() {
      Ok(mode) => {
        system.get_or_init(|| mode);
      }
      Err(error) => tracing::warn!(%error, "cannot read the system appearance"),
    })
  };
  if theme == ThemeSetting::System
    && let Err(error) = runtime.block_on(read)
  {
    tracing::error!(%error, "reading the system appearance failed");
  }
  Appearance::new(theme, system, workspace.cloned())
}

/// What `read` finds in the application database; `None`, logged, when there is no session or it
/// cannot be read.
fn stored<T>(
  workspace: Option<&Workspace>,
  read: impl Fn(&Workspace) -> Result<T, StoreError>,
  what: &str,
) -> Option<T> {
  read(workspace?)
    .inspect_err(|error| tracing::error!(%error, setting = what, "cannot read a setting; using its default"))
    .ok()
}
