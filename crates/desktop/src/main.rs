// A release build is a window without a console: started from Explorer or the Start menu, a
// console program opens a console beside it, and closing that console ends Orchester. Debug
// builds, run with `tools/run.ps1`, keep the console and its log output.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use lurq::app::{App, Tree, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_desktop::{
  activity::Activity,
  app_icon, app_log,
  app_mcp::{AppMcpSetup, DiscoveryFolder},
  app_paths,
  clock::Clock,
  close::{CloseKind, Lifecycle, session_end},
  embedded_mcp::McpStatus,
  i18n,
  messages::Message,
  programs::Programs,
  root::{self, RootProps, Startup},
  runner::{QUIET_STOP_TIMEOUT, Runners},
  secrets::Vault,
  session::{AppSession, StartupError},
  theme,
};
use tokio::runtime::Runtime;

fn main() {
  app_log::install();

  let runtime = Runtime::new().expect("the OS provides threads for the background runtime");
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locale files are valid JSON; covered by tests/i18n.rs");

  // Opened before the window exists, so there is no UI to block yet.
  let session = AppSession::start();
  let startup = startup(&session, &runtime);
  let lifecycle = match &startup {
    Startup::Ready { lifecycle, .. } => Some(lifecycle.clone()),
    Startup::Failed { .. } => None,
  };

  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  // Started before the root mounts so the status bar shows its port, or that it did not start,
  // from the first frame. lurq's server is for looking at the window; the root starts Orchester's own,
  // through which automation controls the app's data, with the window's services.
  let mcp = McpStatus::start(&mut tree);
  let app_mcp = AppMcpSetup::On(DiscoveryFolder::User);
  tree.mount_root::<root::Root>(&mut app, RootProps { startup, mcp, app_mcp });

  #[cfg(feature = "devtools")]
  {
    lurq::app::devtools::load_fonts(&mut app);
    tree.mount_devtools(&mut app);
  }

  let title = app.i18n().t("app.name");
  let icon = app_icon::window_icon().expect("the bundled window icon is a valid PNG; covered by tests/app_icon.rs");
  WinitWindow::new(app, tree)
    .with_title(&title)
    .with_icon(icon)
    // For agents that drive the app over MCP while the user works in other windows.
    .with_start_without_focus(std::env::var_os("ORCHESTER_MCP_START_WITHOUT_FOCUS").is_some_and(|value| value == "1"))
    // A 1440x960 screen between the 32px custom title bar and the 28px status bar; the app
    // draws both bars itself.
    .with_size(1440, 1020)
    .with_min_size(960, 700)
    .with_decorations(false)
    .run();

  // `run` returns once the window has closed; the close flow has normally stopped the runner.
  let (Ok(session), Some(lifecycle)) = (session, lifecycle) else {
    return;
  };
  let end = lifecycle.session_end().unwrap_or_else(|| {
    let outcome = runtime.block_on(lifecycle.runners().shutdown(Some(QUIET_STOP_TIMEOUT)));
    session_end(CloseKind::Quiet, outcome)
  });
  if let Err(error) = session.end(end) {
    tracing::error!(%error, "cannot record the end of the session");
  }
}

fn startup(session: &Result<AppSession, StartupError>, runtime: &Runtime) -> Startup {
  let session = match session {
    Ok(session) => session,
    Err(error) => {
      tracing::error!(%error, "cannot start the session");
      return Startup::Failed {
        message: error.message(),
        path: error.path().map(|path| path.display().to_string()),
      };
    }
  };
  match app_paths::runner_path() {
    Ok(runner) => {
      let activity = Activity::new();
      let runners = Runners::new(
        runner,
        runtime.handle().clone(),
        activity.clone(),
        session.previous_end(),
      );
      Startup::Ready {
        workspace: session.workspace().clone(),
        lifecycle: Lifecycle::new(runners, runtime.handle().clone()),
        activity,
        programs: Programs::processes(),
        vault: Vault::keychain(),
        previous_end: session.previous_end(),
        session_start: session.started_at(),
        clock: Clock::system(),
      }
    }
    Err(error) => {
      tracing::error!(%error, "cannot locate the runner program");
      Startup::Failed {
        message: Message::new("errors.executableLocation"),
        path: None,
      }
    }
  }
}
