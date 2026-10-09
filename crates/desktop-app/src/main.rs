// A release build is a window without a console: started from Explorer or the Start menu, a console
// program opens a console beside it. Debug builds keep the console and its log output.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_app::{
  app_log,
  appearance::{Appearance, ThemeSetting},
  brand, i18n,
  mcp::{self, McpSettings, WindowLink},
  root::{Root, RootProps},
  theme,
};

/// For agents that drive the app over MCP while the person works in other windows: `1` opens the
/// window without taking the focus.
const START_WITHOUT_FOCUS_VARIABLE: &str = "ORCHESTER_APP_MCP_START_WITHOUT_FOCUS";

fn main() {
  app_log::install();

  let appearance = Appearance::new(ThemeSetting::Dark);
  let mut app = App::new();
  theme::install(&mut app, appearance.mode());
  i18n::install(&app).expect("the bundled locale files are valid JSON; covered by tests/i18n.rs");

  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  let link = WindowLink::default();
  // Started before the root mounts, so the status bar shows its port from the first frame.
  let mcp = mcp::start(&mut tree, McpSettings::from_environment(), &link, &appearance);
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
}
