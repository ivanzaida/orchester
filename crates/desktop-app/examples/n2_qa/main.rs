//! N2's states for design verification without a person's press: Kontur on this computer joined to Kontur on a real
//! `orchester-server` on loopback (behind a port the seed can take down, for the offline states), in one of
//! [`State::ALL`]. What only a person does in the app (linking, *Make local*'s confirmation) is done by the seed
//! through the workspace before the window opens ([`seed`]), or by the example's own presses before it shows
//! ([`drive`]); the window keeps its tokens in a keychain in memory, never the OS's. Fixture data for QA, never
//! shipped: an example is no part of the app's program.
//!
//!   cargo run -p orchester-app --example n2_qa -- <state>
//!
//! It prints its pid, its data folder (a new one under `.tmp/qa/n2-states/`, removed when the window closes) and the
//! folder of its MCP discovery file, `.tmp/qa/n2-states/mcp/`. `ORCHESTER_APP_WINDOW_POSITION=<x>,<y>` opens the window
//! there. Stop it by its pid, or close its window.

#![allow(clippy::print_stdout, reason = "a QA helper says where it is")]

mod drive;
#[path = "../../../workspace/tests/sharing_support/forwarder.rs"]
#[allow(dead_code, reason = "the port is taken down, never brought back, in QA")]
mod forwarder;
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

use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  brand, i18n,
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  root::{Root, RootProps, Start},
  startup::{Launch, WindowPosition},
  theme::{self, Mode},
  ui::loader,
};
use orchester_workspace::secrets::Vault;
use tokio::runtime::Runtime;

/// What the window opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
  /// The first link's page for an empty project, five local tasks going up with their numbers (`KSnOW`).
  Upload,
  /// The same page for a checkout without tasks: Kontur's tasks come down.
  Download,
  /// A checkout with tasks and a project with tasks: the join review isn't in this version (`N9NFio`).
  Review,
  /// Linked as a member: Sharing with the members read-only and no Manage (`N9NFio` *as a member*).
  Member,
  /// Linked as an owner: Sharing with *Manage members and invites* (`L5o9Xy`).
  Owner,
  /// Linked as an owner while the server can't be reached: the members as last read (`N9NFio` *server offline*).
  Offline,
  /// *Make local*'s question while the server answers (`woioY`).
  MakeLocal,
  /// *Make local*'s question while it can't be reached (`woioY`).
  MakeLocalOffline,
  /// Removed from the project: Sharing (`xLeTJ`).
  Removed,
  /// The shared task list with its header, synced (`Xr4Wb`, `aocsw`).
  Tasks,
  /// The shared task list's header while the server can't be reached (`aocsw` *offline*).
  TasksOffline,
  /// The shared task list's header once this computer signed out of the server (`aocsw` *signed out*).
  TasksSignedOut,
  /// The shared task list once the person is no longer a member (`sErKV`).
  TasksRemoved,
  /// `orchester.toml` names Kontur on the connected server (`N9NFio` *Discovery file*).
  Discovery,
  /// `orchester.toml` names a server this computer isn't connected to (`oOGMT`).
  DiscoveryUnconnected,
  /// The conflict view of a shared task Alice saved meanwhile (`PenUu`).
  Conflict,
  /// A shared task whose pipeline this computer lacks: its page (*This task*).
  MissingPipeline,
}

impl State {
  const ALL: [(&'static str, Self); 17] = [
    ("upload", Self::Upload),
    ("download", Self::Download),
    ("review", Self::Review),
    ("member", Self::Member),
    ("owner", Self::Owner),
    ("offline", Self::Offline),
    ("make-local", Self::MakeLocal),
    ("make-local-offline", Self::MakeLocalOffline),
    ("removed", Self::Removed),
    ("tasks", Self::Tasks),
    ("tasks-offline", Self::TasksOffline),
    ("tasks-signed-out", Self::TasksSignedOut),
    ("tasks-removed", Self::TasksRemoved),
    ("discovery", Self::Discovery),
    ("discovery-unconnected", Self::DiscoveryUnconnected),
    ("conflict", Self::Conflict),
    ("missing-pipeline", Self::MissingPipeline),
  ];

  fn named(name: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .find(|(known, _)| *known == name)
      .map(|(_, state)| *state)
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let name = std::env::args().nth(1).unwrap_or_else(|| "owner".to_owned());
  let state = State::named(&name).ok_or_else(|| {
    let names: Vec<&str> = State::ALL.iter().map(|(name, _)| *name).collect();
    format!("expected one of {}", names.join(", "))
  })?;
  let position = WindowPosition::from_environment()?;
  let scratch = qa_folder()?;
  std::fs::create_dir_all(&scratch)?;
  let data = tempfile::Builder::new().prefix("app-").tempdir_in(&scratch)?;
  let runtime = Runtime::new()?;
  let launch = Launch::begin_with_vault(
    &data.path().join("orchester.db"),
    data.path().join("no-runner"),
    runtime.handle(),
    (None, Vault::with(Arc::new(vault::QaVault::default()))),
  );
  let services = launch.services().ok_or("the QA session did not start")?.clone();
  let workspace = launch.workspace().ok_or("the QA session did not start")?.clone();
  let seeded = seed::seed(state, (&services, &workspace), (&runtime, data.path()))?;
  let start = match launch.start() {
    Start::Ready(mut ready) => {
      ready.landing.route = seeded.route.clone();
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
  window(&runtime, (&launch, start), (position, state, &seeded))?;
  launch.end();
  Ok(())
}

/// The window on `start` until it closes, with the app's MCP server in QA mode, after the example's own presses
/// for `state` ([`drive::before_showing`]).
fn window(
  runtime: &Runtime,
  (launch, start): (&Launch, Start),
  (position, state, seeded): (Option<WindowPosition>, State, &seed::Seeded),
) -> Result<(), Box<dyn Error>> {
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::new(OnceLock::new()), None);
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app)?;
  let mut tree = Tree::new();
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
  tree.resize(1440, 900);
  drive::before_showing(&mut app, &mut tree, (state, seeded))?;
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  let window = WinitWindow::new(app, tree)
    .with_title("Orchester N2 QA")
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

/// `.tmp/qa/n2-states/` of this checkout.
fn qa_folder() -> Result<PathBuf, &'static str> {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .ancestors()
    .nth(2)
    .map(|checkout| checkout.join(".tmp").join("qa").join("n2-states"))
    .ok_or("the crate is two folders into the checkout")
}
