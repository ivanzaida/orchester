//! N1's states for design verification without a person's press (N1 design 2): Settings › Orchester signed in to a
//! stand-in Orchester Cloud, with an invite to Billing on a real `orchester-server` on loopback, in one of
//! [`State::ALL`]. Signing in and accepting are a person's in the app, so the seed does them through the workspace
//! before the window opens ([`seed`]); the window keeps its tokens in a keychain in memory ([`vault`]), never the OS's.
//! Fixture data for QA, never shipped: an example is no part of the app's program.
//!
//!   cargo run -p orchester-app --example n1_qa -- <state>
//!
//! It prints its pid, its data folder (a new one under `.tmp/qa/n1-states/`, removed when the window closes) and the
//! folder of its MCP discovery file, `.tmp/qa/n1-states/mcp/`. `ORCHESTER_APP_WINDOW_POSITION=<x>,<y>` opens the window
//! there. Stop it by its pid, or close its window.

#![allow(clippy::print_stdout, reason = "a QA helper says where it is")]

#[path = "../../../workspace/tests/sharing_support/account_cloud.rs"]
mod account_cloud;
mod backends;
mod seed;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;
#[path = "../secrets_qa/vault.rs"]
#[allow(dead_code, reason = "the secrets QA's delays and failures are not used here")]
mod vault;

use std::{
  error::Error,
  path::{Path, PathBuf},
  sync::{Arc, OnceLock},
};

use backends::Backends;
use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  brand, i18n,
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  root::{Root, RootProps, Start},
  routes::{OrchesterView, Route},
  startup::{Launch, WindowPosition},
  theme::{self, Mode},
  ui::loader,
};
use orchester_workspace::secrets::Vault;
use tokio::runtime::Runtime;

/// What the window opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
  /// Signed in, with the Billing invite open (`DUWM5`'s signed-in side, *Invites* `s25gc8`).
  SignedIn,
  /// Accepted, and nothing asked for it yet: *Joining … didn't finish* (`DheFE`).
  Pending,
  /// Accepted, and its join failed after the cloud's token: *Joining … didn't finish* with *Try again* (`DheFE`).
  Failed,
  /// Waiting for approval (`Bl08B`), asked again on its own after 2, 4 and 8 minutes.
  Waiting,
  /// Waited, and Orchester stopped asking on its own: *Try again* (`DheFE`, the unanswered words).
  Unanswered,
  /// Asked for as often as Orchester does: *Cancel joining* instead of *Try again*.
  TooMany,
  /// Approved while it waited: the check finishes the join as the window opens (`P0wvD`).
  Approved,
  /// Refused while it waited: the check finds it as the window opens (`S5mMY`).
  Refused,
}

impl State {
  const ALL: [(&'static str, Self); 8] = [
    ("signed-in", Self::SignedIn),
    ("pending", Self::Pending),
    ("failed", Self::Failed),
    ("waiting", Self::Waiting),
    ("unanswered", Self::Unanswered),
    ("too-many", Self::TooMany),
    ("approved", Self::Approved),
    ("refused", Self::Refused),
  ];

  fn named(name: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .find(|(known, _)| *known == name)
      .map(|(_, state)| *state)
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let name = std::env::args().nth(1).unwrap_or_else(|| "waiting".to_owned());
  let state = State::named(&name).ok_or_else(|| {
    let names: Vec<&str> = State::ALL.iter().map(|(name, _)| *name).collect();
    format!("expected one of {}", names.join(", "))
  })?;
  let position = WindowPosition::from_environment()?;
  let scratch = qa_folder()?;
  std::fs::create_dir_all(&scratch)?;
  let data = tempfile::Builder::new().prefix("app-").tempdir_in(&scratch)?;
  let other = data.path().join("other-computer");
  std::fs::create_dir_all(&other)?;
  let backends = Backends::start(true);
  let runtime = Runtime::new()?;
  let launch = Launch::begin_with_vault(
    &data.path().join("orchester.db"),
    data.path().join("no-runner"),
    runtime.handle(),
    (None, Vault::with(Arc::new(vault::QaVault::default()))),
  );
  let services = launch.services().ok_or("the QA session did not start")?.clone();
  let database = data.path().join("orchester.db");
  let panel = seed::seed(state, &backends, &services, (&runtime, &database, &other))?;
  let start = match launch.start() {
    Start::Ready(mut ready) => {
      ready.landing.route = Route::Orchester(OrchesterView { server: None, panel });
      Start::Ready(ready)
    }
    problem @ Start::Problem(_) => problem,
  };
  println!(
    "pid {}\nmcp {}\ndata {}\nstate {name}",
    std::process::id(),
    scratch.join("mcp").display(),
    data.path().display()
  );
  window(&runtime, (&launch, start), position)?;
  launch.end();
  Ok(())
}

/// The window on `start` until it closes, with the app's MCP server in QA mode.
fn window(
  runtime: &Runtime,
  (launch, start): (&Launch, Start),
  position: Option<WindowPosition>,
) -> Result<(), Box<dyn Error>> {
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::new(OnceLock::new()), None);
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app)?;
  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  loader::install(&mut tree);
  let link = WindowLink::default();
  let settings = McpSettings::with_window_control(true);
  let config = mcp::config(settings, &link, &appearance, launch.services(), app.i18n());
  let server = AppMcp::start(
    &mut tree,
    runtime.handle(),
    McpMode::Qa(qa_folder()?.join("mcp")),
    (config, settings),
    &link.input_gate(),
  );
  tree.mount_root::<Root>(
    &mut app,
    RootProps {
      link,
      mcp: McpLink::new(&server),
      start,
    },
  );
  let window = WinitWindow::new(app, tree)
    .with_title("Orchester N1 QA")
    .with_icon(brand::window_icon()?)
    .with_size(1440, 900)
    .with_min_size(960, 640)
    .with_start_without_focus(true)
    .with_decorations(false)
    .with_corner_radius(WindowCornerRadius::Rounded);
  match position {
    Some(WindowPosition { x, y }) => window.with_position(x, y),
    None => window,
  }
  .run();
  server.shutdown();
  Ok(())
}

/// `.tmp/qa/n1-states/` of this checkout.
fn qa_folder() -> Result<PathBuf, &'static str> {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .ancestors()
    .nth(2)
    .map(|checkout| checkout.join(".tmp").join("qa").join("n1-states"))
    .ok_or("the crate is two folders into the checkout")
}
