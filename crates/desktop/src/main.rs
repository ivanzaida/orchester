use lurq::app::{App, Tree, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_desktop::{
  i18n,
  root::{self, Startup},
  session::AppSession,
  theme::{self, ExtraColor},
};
use tracing_subscriber::EnvFilter;

fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
    .init();

  let runtime = tokio::runtime::Runtime::new().expect("the OS provides threads for the background runtime");
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locale files are valid JSON; covered by tests/i18n.rs");

  // Opened before the window exists, so there is no UI to block yet.
  let session = AppSession::start();
  let startup = match &session {
    Ok(session) => Startup::Ready(session.workspace().clone()),
    Err(error) => {
      tracing::error!(%error, "cannot start the session");
      Startup::Failed {
        message: error.message(),
        path: error.path().map(|path| path.display().to_string()),
      }
    }
  };

  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  tree.mount_root::<root::Root>(&mut app, startup);

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
  let title_bar = app.theme().palette_color(ExtraColor::Surface.role());
  WinitWindow::new(app, tree)
    .with_title(&title)
    .with_size(1440, 960)
    .with_min_size(960, 640)
    .with_title_bar_color(title_bar)
    .run();

  // `run` returns once the window has closed normally.
  if let Ok(session) = session
    && let Err(error) = session.end()
  {
    tracing::error!(%error, "cannot record the end of the session");
  }
}
