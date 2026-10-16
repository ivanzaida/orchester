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
mod project;
mod seed;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;
mod two_factor;
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
  /// Kontur registered here and Kontur team joined, with the Billing invite waiting: All projects and its notice.
  Project,
  /// The same, at Project settings › Sharing asking *Link and upload* in place.
  LinkConfirm,
  /// The same with Kontur linked: Project settings › Sharing as `L5o9Xy`, with *Leave*.
  Linked,
  /// The same, then signed out and in as another Orchester Cloud account: Settings › Orchester and the status bar
  /// show none of the first account's servers (servers per account).
  OtherAccount,
  /// Two-factor authentication off, the Billing invite waiting: *Turn on…* in the header, *Turn on two-factor
  /// authentication* on the invite (`hC11x`, `qwDH9` cell 1).
  TwoFactorOff,
  /// Signed in without a code, *Accept* pressed and refused: *Sign in again to accept* (`rPG8h`).
  SignInAgain,
  /// Signed in without a code: *Sign in again* in *Accept*'s place, as after *Not now* (`qwDH9` cell 3).
  NotNow,
  /// All projects' notice with two-factor authentication off (`qwDH9` cell 2).
  NoticeOff,
  /// All projects' notice after its *Accept* was refused for want of a code (`qwDH9` cell 4).
  NoticeRefused,
  /// Signed in with a recovery code that left one: the warning in Settings › Orchester (no frame).
  FewRecoveryCodes,
}

impl State {
  const ALL: [(&'static str, Self); 18] = [
    ("signed-in", Self::SignedIn),
    ("pending", Self::Pending),
    ("failed", Self::Failed),
    ("waiting", Self::Waiting),
    ("unanswered", Self::Unanswered),
    ("too-many", Self::TooMany),
    ("approved", Self::Approved),
    ("refused", Self::Refused),
    ("project", Self::Project),
    ("link-confirm", Self::LinkConfirm),
    ("linked", Self::Linked),
    ("other-account", Self::OtherAccount),
    ("two-factor-off", Self::TwoFactorOff),
    ("sign-in-again", Self::SignInAgain),
    ("not-now", Self::NotNow),
    ("notice-off", Self::NoticeOff),
    ("notice-refused", Self::NoticeRefused),
    ("few-recovery-codes", Self::FewRecoveryCodes),
  ];

  fn named(name: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .find(|(known, _)| *known == name)
      .map(|(_, state)| *state)
  }

  const fn two_factor(self) -> bool {
    matches!(
      self,
      Self::TwoFactorOff
        | Self::SignInAgain
        | Self::NotNow
        | Self::NoticeOff
        | Self::NoticeRefused
        | Self::FewRecoveryCodes
    )
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
  std::fs::create_dir_all(data.path().join("other-computer"))?;
  let backends = Backends::start(true);
  let runtime = Runtime::new()?;
  let launch = Launch::begin_with_vault(
    &data.path().join("orchester.db"),
    data.path().join("no-runner"),
    runtime.handle(),
    (None, Vault::with(Arc::new(vault::QaVault::default()))),
  );
  let services = launch.services().ok_or("the QA session did not start")?.clone();
  let workspace = launch.workspace().ok_or("the QA session did not start")?.clone();
  let (route, press) = if state.two_factor() {
    seed::sign_in(&backends, &services, &runtime)?;
    two_factor::seed(state, (&backends, &services, &workspace), (&runtime, data.path()))?
  } else {
    (
      seed::seed(state, (&backends, &services, &workspace), (&runtime, data.path()))?,
      None,
    )
  };
  let start = match launch.start() {
    Start::Ready(mut ready) => {
      ready.landing.route = route;
      Start::Ready(ready)
    }
    problem @ Start::Problem(_) => problem,
  };
  // Sharing opens on its tab; *Link and upload* asks after the person's *Link*, which the example presses before the
  // window shows.
  let sharing = matches!(state, State::LinkConfirm | State::Linked).then(|| (state, backends.server.url()));
  println!(
    "pid {}\nmcp {}\ndata {}\nstate {name}",
    std::process::id(),
    scratch.join("mcp").display(),
    data.path().display()
  );
  let press = press.as_ref().map(|press| (press, &backends.cloud));
  window(&runtime, (&launch, start), (position, sharing, press))?;
  launch.end();
  Ok(())
}

/// A press of a two-factor state, at the stand-in cloud whose session it changes.
type Pressing<'a> = Option<(&'a two_factor::Press, &'a account_cloud::AccountCloud)>;

/// The window on `start` until it closes, with the app's MCP server in QA mode; `sharing`: the state to open
/// Project settings › Sharing for, and the server ([`project::open_sharing`]); `press`: a press of a two-factor state
/// ([`two_factor::drive`]).
fn window(
  runtime: &Runtime,
  (launch, start): (&Launch, Start),
  (position, sharing, press): (Option<WindowPosition>, Option<(State, String)>, Pressing<'_>),
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
  if let Some((state, server)) = sharing {
    tree.resize(1440, 900);
    project::open_sharing(&mut app, &mut tree, (state, &server))?;
  }
  if let Some(press) = press {
    tree.resize(1440, 900);
    two_factor::drive(&mut app, &mut tree, press)?;
  }
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
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
