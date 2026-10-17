//! N3's states for design verification without a person's press: Kontur on this computer and KTR, Kontur's project on
//! a real `orchester-server` on loopback (behind a port the seed can take down), in one of [`State::ALL`]. What only a
//! person does in the app (joining, *Share task*) is done by the seed through the workspace before the window opens
//! ([`seed`]), or by the example's own presses before it shows ([`drive`]); the window keeps its tokens in a keychain
//! in memory, never the OS's. Its folder picker hands over the state's folder and Copy writes to `copied.txt` in its
//! data folder, never the clipboard. It signs in past the sign-in gate to a stand-in cloud on loopback first, as the
//! other QA examples do. Fixture data for QA, never shipped: an example is no part of the app's program.
//!
//!   cargo run -p orchester-app --example n3_qa -- <state>
//!
//! It prints its pid, its data folder (a new one under `.tmp/qa/n3-states/`, removed when the window closes) and the
//! folder of its MCP discovery file, `.tmp/qa/n3-states/mcp/`. `ORCHESTER_APP_WINDOW_POSITION=<x>,<y>` opens the window
//! there. Stop it by its pid, or close its window.

#![allow(clippy::print_stdout, reason = "a QA helper says where it is")]

mod drive;
#[path = "../../../workspace/tests/sharing_support/forwarder.rs"]
#[allow(dead_code, reason = "the port is taken down, never brought back, in QA")]
mod forwarder;
#[path = "../projects_qa/host.rs"]
mod host;
#[path = "../../tests/project_settings/run_fixture.rs"]
mod run_fixture;
mod seed;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;
#[path = "../../tests/support/signed_in.rs"]
mod signed_in;
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
  root::{RootProps, Start},
  startup::{Launch, WindowPosition},
  theme::{self, Mode},
  ui::loader,
};
use orchester_workspace::secrets::Vault;
use tokio::runtime::Runtime;

/// What the window opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
  /// The join review, every task ticked; KON-2's run hasn't ended (`AiCwO`).
  JoinReview,
  /// The join review with KON-1 and KON-5 unticked: they stay Not shared.
  JoinUnticked,
  /// The join review after a join that couldn't reach the server: why, with the ticks kept.
  JoinFailed,
  /// The join review after an MCP client pressed *Join and upload*: the person-only note.
  JoinPersonOnly,
  /// Sharing after the join: *You joined KTR on Kontur team* and *Keys before joining* (`oOGMT`).
  Joined,
  /// *Link to this project…* for a project the person isn't a member of: *No project you're a member of here*
  /// (`oOGMT`).
  NotMember,
  /// The task list after the join: *was KON-…* and *Not shared · 2* (`Xr4Wb`).
  Tasks,
  /// The task list with the Not shared filter on (`A7PKfL`).
  TasksNotShared,
  /// The task list searched by a key before joining, KON-3 (`T1ahf8`).
  TasksFormerKey,
  /// A search that finds only a Not shared task, KON-5 (`T1ahf8`).
  TasksOnlyNotShared,
  /// The Not shared filter on, with a search (`T1ahf8`).
  TasksNotSharedSearch,
  /// The task list after KON-5 was shared later: its new key with *was KON-5*, and *Not shared · 1* (`T1ahf8`).
  TasksSharedLater,
  /// The page of KON-1, kept Not shared (`R4elt`).
  TaskNotShared,
  /// *Share KON-1 on KTR?* asked in place (`TfVtK`).
  TaskShare,
  /// The page of a task that went up as a KTR key, *was KON-3* (`VuLYC`).
  TaskJoined,
  /// The invite landing with two checkouts to pick from, one a fork (`xeFRB`).
  Landing,
  /// The invite landing without a checkout of the project: the `git clone` to run (`d8oDsO`).
  LandingNone,
  /// The invite landing after a checkout folder was added: *Added* and the match (`d8oDsO`).
  LandingAdded,
  /// The invite landing after a folder that isn't a git checkout was picked (`d8oDsO`).
  LandingNotGit,
  /// The invite landing after a folder linked to another project was picked (`d8oDsO`).
  LandingLinked,
  /// The invite landing with the one checkout that matches: it opens the join review at once (`d8oDsO`).
  LandingDirect,
  /// *Later*: the server's entry in Settings › Orchester, KTR with *Link a checkout…* (`d8oDsO`).
  Later,
}

impl State {
  const ALL: [(&'static str, Self); 22] = [
    ("join-review", Self::JoinReview),
    ("join-unticked", Self::JoinUnticked),
    ("join-failed", Self::JoinFailed),
    ("join-person-only", Self::JoinPersonOnly),
    ("joined", Self::Joined),
    ("not-member", Self::NotMember),
    ("tasks", Self::Tasks),
    ("tasks-not-shared", Self::TasksNotShared),
    ("tasks-former-key", Self::TasksFormerKey),
    ("tasks-only-not-shared", Self::TasksOnlyNotShared),
    ("tasks-not-shared-search", Self::TasksNotSharedSearch),
    ("tasks-shared-later", Self::TasksSharedLater),
    ("task-not-shared", Self::TaskNotShared),
    ("task-share", Self::TaskShare),
    ("task-joined", Self::TaskJoined),
    ("landing", Self::Landing),
    ("landing-none", Self::LandingNone),
    ("landing-added", Self::LandingAdded),
    ("landing-not-git", Self::LandingNotGit),
    ("landing-linked", Self::LandingLinked),
    ("landing-direct", Self::LandingDirect),
    ("later", Self::Later),
  ];

  fn named(name: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .find(|(known, _)| *known == name)
      .map(|(_, state)| *state)
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let name = std::env::args().nth(1).unwrap_or_else(|| "join-review".to_owned());
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
  let cloud = signed_in::account_cloud::AccountCloud::start();
  signed_in::sign_in(&services, runtime.handle(), &cloud);
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
  window(&runtime, (&launch, start), (position, state, &seeded), data.path())?;
  launch.end();
  Ok(())
}

/// The window on `start` until it closes, with the app's MCP server in QA mode, after the example's own presses
/// for `state` ([`drive::before_showing`]).
fn window(
  runtime: &Runtime,
  (launch, start): (&Launch, Start),
  (position, state, seeded): (Option<WindowPosition>, State, &seed::Seeded),
  data: &Path,
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
  let root = RootProps {
    link,
    mcp: McpLink::new(&server),
    start,
  };
  tree.mount_root::<host::Host>(
    &mut app,
    host::props(
      root,
      seeded.picked.clone(),
      data.join("no-program"),
      data.join("copied.txt"),
    ),
  );
  tree.resize(1440, 900);
  drive::before_showing(&mut app, &mut tree, (state, seeded))?;
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  let window = WinitWindow::new(app, tree)
    .with_title("Orchester N3 QA")
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

/// `.tmp/qa/n3-states/` of this checkout.
fn qa_folder() -> Result<PathBuf, &'static str> {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .ancestors()
    .nth(2)
    .map(|checkout| checkout.join(".tmp").join("qa").join("n3-states"))
    .ok_or("the crate is two folders into the checkout")
}
