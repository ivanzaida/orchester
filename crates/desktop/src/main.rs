use lurq::app::{App, Tree, theme::PaletteColor, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_desktop::{i18n, root, theme};
use tracing_subscriber::EnvFilter;

fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
    .init();

  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locale files are valid JSON; covered by tests/i18n.rs");

  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  tree.mount_root::<root::Root>(&mut app, ());

  #[cfg(feature = "devtools")]
  {
    lurq::app::devtools::load_fonts(&mut app);
    tree.mount_devtools(&mut app);
  }

  #[cfg(feature = "mcp")]
  let _mcp = {
    let mcp = tree.enable_mcp(lurq::mcp::McpConfig::new().app_name("orchester"));
    tracing::info!("MCP server on http://127.0.0.1:{}/mcp", mcp.port());
    mcp
  };

  let title = app.i18n().t("app.name");
  let title_bar = app.theme().palette_color(PaletteColor::SurfaceBase);
  WinitWindow::new(app, tree)
    .with_title(&title)
    .with_size(1440, 960)
    .with_min_size(960, 640)
    .with_title_bar_color(title_bar)
    .run();
}
