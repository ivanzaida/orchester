//! Settings › Automation › *Orchester MCP server* as the installed app shows it, for design verification:
//! `cargo run -p orchester-app --example mcp_settings_qa -- [listening|port-taken|off|token-unreadable]`.
//!
//! Every launch keeps its data under `.tmp/qa/mcp-settings/`; the OS keychain, the installed app's port and its token
//! are never used. Two servers run: the one Settings and the status bar show, as the installed app's (its port a
//! free one this harness may hold to show it taken, its token in memory), whose tools are hosted on a tree of their
//! own; and a QA copy's on the window, whose discovery file the verifier connects with (`call_run_app` or
//! `tools/lurq-mcp-client.py --discovery .tmp/qa/mcp-settings/mcp`), so it can take screenshots in every state.
//! Controls of the section take a person's click only, so states that need one (the Codex tab, *Rotate the
//! token?*, the token shown, Copied) are covered by `tests/settings_mcp` instead.

#[path = "../../tests/support/signed_in.rs"]
mod signed_in;
mod vault;

use std::{
  net::TcpListener,
  path::PathBuf,
  sync::{Arc, OnceLock},
};

use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  brand, i18n,
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  project::Landing,
  root::{Ready, Root, RootProps, Start},
  routes::Route,
  services::Services,
  theme::{self, Mode},
  ui::loader,
};
use orchester_core::app_mcp::McpPort;
use orchester_store::SessionEnd;
use orchester_workspace::{secrets::Vault, session::AppSession, workspace::Workspace};
use tokio::runtime::Runtime;

const STATES: [&str; 4] = ["listening", "port-taken", "off", "token-unreadable"];

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let state = std::env::args().nth(1).unwrap_or_else(|| "listening".into());
  if !STATES.contains(&state.as_str()) {
    return Err(format!("expected one of {}", STATES.join(", ")).into());
  }
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/qa/mcp-settings");
  std::fs::create_dir_all(&scratch)?;
  let data = tempfile::Builder::new().prefix("app-").tempdir_in(&scratch)?;
  let runtime = Runtime::new()?;
  let session = AppSession::start_at(&data.path().join("orchester.db"))?;
  let workspace = session.workspace().clone();
  let Installation { port, taken, vault } = installation(&state, &workspace)?;
  // A keychain in memory, signed in past the sign-in gate to a stand-in cloud that answers while the window is open.
  let services = Services::start(
    &session,
    data.path().join("no-runner"),
    runtime.handle().clone(),
    signed_in::keychain(),
  );
  let cloud = signed_in::account_cloud::AccountCloud::start();
  signed_in::sign_in(&services, runtime.handle(), &cloud);
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::new(OnceLock::new()), None);
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app)?;
  let link = WindowLink::default();
  let settings = McpSettings::with_window_control(true);
  // Started first: the gate denies input through the window's tools, which the QA copy's server attaches last.
  let mut elsewhere = Tree::new();
  let shown = AppMcp::start(
    &mut elsewhere,
    runtime.handle(),
    McpMode::Installation {
      workspace,
      vault: Vault::with(vault),
    },
    (mcp::config(settings, &link, &appearance, None, app.i18n()), settings),
    &link.input_gate(),
  );
  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  loader::install(&mut tree);
  let discovery = scratch.join("mcp");
  let config = mcp::config(settings, &link, &appearance, Some(&services), app.i18n());
  let verifier = AppMcp::start(
    &mut tree,
    runtime.handle(),
    McpMode::Qa(discovery.clone()),
    (config, settings),
    &link.input_gate(),
  );
  tree.mount_root::<Root>(
    &mut app,
    RootProps {
      link,
      mcp: McpLink::new(&shown),
      start: Start::Ready(Box::new(Ready {
        services,
        landing: Landing {
          route: Route::Settings,
          project: None,
        },
      })),
    },
  );
  use std::io::Write as _;
  writeln!(
    std::io::stdout().lock(),
    "pid {}\nmcp {}\nshown port {port}",
    std::process::id(),
    discovery.display()
  )?;
  run(app, tree)?;
  verifier.shutdown();
  shown.shutdown();
  drop(taken);
  session.end(SessionEnd::Clean)?;
  Ok(())
}

/// The installation `state` asks for: a free port of this computer, held by this harness when it should show as
/// taken, the server on or off, and a store in memory that refuses reads when the token should be unreadable.
fn installation(state: &str, workspace: &Workspace) -> Result<Installation, Box<dyn std::error::Error>> {
  let holder = TcpListener::bind(("127.0.0.1", 0))?;
  let port = holder.local_addr()?.port();
  workspace.set_mcp_port(McpPort::new(u32::from(port))?)?;
  workspace.set_mcp_server_enabled(state != "off")?;
  let vault = Arc::new(vault::MemoryVault::default());
  vault.refuse_reads(state == "token-unreadable");
  Ok(Installation {
    port,
    taken: (state == "port-taken").then_some(holder),
    vault,
  })
}

/// The installation Settings shows.
struct Installation {
  port: u16,
  /// The port, held by this harness while it should show as taken.
  taken: Option<TcpListener>,
  vault: Arc<vault::MemoryVault>,
}

/// The window, at the size of `lye6f`, without taking the focus; returns once it closed.
fn run(app: App, tree: Tree) -> Result<(), Box<dyn std::error::Error>> {
  WinitWindow::new(app, tree)
    .with_title("Orchester MCP settings QA")
    .with_icon(brand::window_icon()?)
    .with_size(1440, 1320)
    .with_min_size(960, 640)
    .with_start_without_focus(true)
    .with_decorations(false)
    .with_corner_radius(WindowCornerRadius::Rounded)
    .run();
  Ok(())
}
