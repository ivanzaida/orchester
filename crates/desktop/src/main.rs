use lurq::app::{App, Tree, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_desktop::{
  activity::Activity,
  app_log, app_paths,
  close::{CloseKind, Lifecycle, session_end},
  i18n,
  messages::Message,
  programs::Programs,
  root::{self, Startup},
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
  WinitWindow::new(app, tree)
    .with_title(&title)
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
