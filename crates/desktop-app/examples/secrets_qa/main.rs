//! Native Secrets QA: cargo run -p orchester-app --example secrets_qa -- [list|writes|empty].
//! Every launch creates its own data under .tmp/qa/secrets-native; no OS keychain or runner is used.
//! The example-only qa_secrets MCP tool controls actual vault delay/failure and masked fixture input.
//! The same Root, screen, workspace actions, MCP privacy and person-only gates as the app are used.
mod fixtures;
#[path = "../../tests/support/signed_in.rs"]
mod signed_in;
mod tools;
mod vault;

use std::{
  path::PathBuf,
  sync::{Arc, OnceLock},
};

use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  brand, i18n,
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  project::{Landing, ProjectState},
  root::{Ready, Root, RootProps, Start},
  routes::{ProjectPage, Route},
  services::Services,
  theme::{self, Mode},
  ui::loader,
};
use orchester_store::SessionEnd;
use orchester_workspace::{secrets::Vault, session::AppSession};
use tokio::runtime::Runtime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let scenario = std::env::args().nth(1).unwrap_or_else(|| "list".into());
  if !["list", "writes", "empty"].contains(&scenario.as_str()) {
    return Err("expected list, writes or empty".into());
  }
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/qa/secrets-native");
  std::fs::create_dir_all(&scratch)?;
  let data = tempfile::Builder::new().prefix("app-").tempdir_in(scratch)?;
  let runtime = Runtime::new()?;
  let session = AppSession::start_at(&data.path().join("orchester.db"))?;
  let vault = Arc::new(vault::QaVault::default());
  let open = fixtures::seed(session.workspace(), &data.path().join("repo"), &vault, &scenario)?;
  let services = Services::start(
    &session,
    data.path().join("no-runner"),
    runtime.handle().clone(),
    Vault::with(vault.clone()),
  );
  // Past the sign-in gate, as a person who signed in; the stand-in cloud answers while the window is open.
  let cloud = signed_in::account_cloud::AccountCloud::start();
  signed_in::sign_in(&services, runtime.handle(), &cloud);
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::new(OnceLock::new()), None);
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app)?;
  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  loader::install(&mut tree);
  let link = WindowLink::default();
  let settings = McpSettings::of_build().with_environment();
  let config = mcp::config(settings, &link, &appearance, Some(&services), app.i18n()).tool(tools::tool(
    vault.clone(),
    open.clone(),
    services.clone(),
    link.input_gate(),
  ));
  let discovery = data.path().join("discovery");
  let server = AppMcp::start(
    &mut tree,
    runtime.handle(),
    McpMode::Qa(discovery.clone()),
    (config, settings),
    &link.input_gate(),
  );
  let status = McpLink::new(&server);
  tree.mount_root::<Root>(
    &mut app,
    RootProps {
      link,
      mcp: status,
      start: Start::Ready(Box::new(Ready {
        services,
        landing: Landing {
          route: Route::Project(open.project.id(), ProjectPage::Secrets),
          project: Some((open.project.id(), ProjectState::Open(open))),
        },
      })),
    },
  );
  use std::io::Write as _;
  writeln!(
    std::io::stdout().lock(),
    "pid {}\ndiscovery {}",
    std::process::id(),
    discovery.display()
  )?;
  WinitWindow::new(app, tree)
    .with_title("Orchester Secrets QA")
    .with_icon(brand::window_icon()?)
    .with_size(1440, 900)
    .with_min_size(960, 640)
    .with_start_without_focus(true)
    .with_decorations(false)
    .with_corner_radius(WindowCornerRadius::Rounded)
    .run();
  vault.configure(false, 0);
  server.shutdown();
  session.end(SessionEnd::Clean)?;
  Ok(())
}
