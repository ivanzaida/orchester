// A release build is a window without a console: started from Explorer or the Start menu, a console
// program opens a console beside it. Debug builds keep the console and its log output.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_app::{
  app_log,
  appearance::{Appearance, system},
  brand, i18n,
  mcp::{self, AppMcp, DiscoveryFolder, McpSettings, McpStatus, WindowLink},
  root::{Root, RootProps},
  settings::AppSettings,
  theme,
  ui::loader,
};
use orchester_store::StoreError;
use tokio::runtime::Runtime;

/// For agents that drive the app over MCP while the person works in other windows: `1` opens the
/// window without taking the focus.
const START_WITHOUT_FOCUS_VARIABLE: &str = "ORCHESTER_APP_MCP_START_WITHOUT_FOCUS";

fn main() {
  app_log::install();

  let runtime = Runtime::new().expect("the OS provides threads for the background runtime");
  let settings = AppSettings::open_app_database()
    .inspect_err(|error| tracing::error!(%error, "the app runs without its settings; nothing it changes is kept"))
    .ok();
  let appearance = appearance(&runtime, settings.as_ref());
  let mut app = App::new();
  theme::install(&mut app, appearance.mode());
  i18n::install(&app).expect("the bundled locale files are valid JSON; covered by tests/i18n.rs");

  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  loader::install(&mut tree);
  let link = WindowLink::default();
  let window_control = stored(settings.as_ref(), AppSettings::mcp_window_control, "window control").flatten();
  let mcp_settings = McpSettings::of_build().with_stored(window_control).with_environment();
  // Started before the root mounts, so the status bar shows its port from the first frame.
  let config = mcp::config(mcp_settings, &link, &appearance);
  let server = AppMcp::start(&mut tree, runtime.handle(), &DiscoveryFolder::User, config);
  let mcp = McpStatus::of(server.as_ref());
  tree.mount_root::<Root>(&mut app, RootProps { link, mcp });

  #[cfg(feature = "devtools")]
  {
    lurq::app::devtools::load_fonts(&mut app);
    tree.mount_devtools(&mut app);
  }

  let title = app.i18n().t("app.name");
  let icon = brand::window_icon().expect("the bundled window icon is a valid PNG; covered by tests/brand.rs");
  let without_focus = std::env::var_os(START_WITHOUT_FOCUS_VARIABLE).is_some_and(|value| value == "1");
  WinitWindow::new(app, tree)
    .with_title(&title)
    .with_icon(icon)
    .with_start_without_focus(without_focus)
    // The window frames of the design (`Tasks · dark` `jn2IT`), title bar and status bar included.
    .with_size(1440, 900)
    .with_min_size(960, 640)
    // The app draws its own title bar and frame (`chrome`); the OS keeps the rounded corners.
    .with_decorations(false)
    .with_corner_radius(WindowCornerRadius::Rounded)
    .run();

  // `run` returns once the window has closed: the server stops and its discovery file goes.
  drop(server);
}

/// The stored theme, resolved with the operating system's appearance, which is read on the runtime's
/// blocking pool, never on the UI thread.
fn appearance(runtime: &Runtime, settings: Option<&AppSettings>) -> Appearance {
  let system = match runtime.block_on(runtime.spawn_blocking(system::mode)) {
    Ok(Ok(mode)) => Some(mode),
    Ok(Err(error)) => {
      tracing::warn!(%error, "cannot read the system appearance");
      None
    }
    Err(error) => {
      tracing::error!(%error, "reading the system appearance failed");
      None
    }
  };
  let theme = stored(settings, AppSettings::theme, "theme").unwrap_or_default();
  Appearance::new(theme, system, settings.cloned())
}

/// What `read` finds in the settings; `None`, logged, when they are not open or cannot be read.
fn stored<T>(
  settings: Option<&AppSettings>,
  read: impl Fn(&AppSettings) -> Result<T, StoreError>,
  what: &str,
) -> Option<T> {
  read(settings?)
    .inspect_err(|error| tracing::error!(%error, setting = what, "cannot read a setting; using its default"))
    .ok()
}
