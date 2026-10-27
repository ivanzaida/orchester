//! The sign-in gate's states for design verification without a person's press (layer *Sign-in gate* `ASl21`): the
//! window opens in one of [`State::ALL`] at 1440 or, with `--narrow`, 960 wide. What a person would have done is done
//! before the window shows: signing in, the cloud ending the sign-in, or typing and pressing *Sign in* with the pointer
//! ([`drive`]); with `--tab <count>`, Tab pressed that many times after, so a focus ring shows. The window keeps its
//! tokens in a keychain in memory, and its cloud is a stand-in on loopback. Fixture data for QA, never shipped: an
//! example is no part of the app's program.
//!
//!   ORCHESTER_CLOUD_URL=http://localhost:47990 cargo run -p orchester-app --example gate_qa -- <state> [--narrow] [--tab <count>]
//!
//! Built that way, the stand-in is the build's own cloud, as the frames draw it (*default*, no test cloud); built
//! without it, the stand-in is set as a test cloud, and the gate says so. It prints its pid and the folder of its MCP
//! discovery file, `.tmp/qa/signin-gate/mcp/`. `ORCHESTER_APP_WINDOW_POSITION=<x>,<y>` opens the window there. Stop it
//! by its pid, or close its window.

#![allow(clippy::print_stdout, reason = "a QA helper says where it is")]

mod drive;

use std::{
  error::Error,
  path::{Path, PathBuf},
  sync::{Arc, OnceLock},
};

use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_api::client::ServerAddress;
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  brand, i18n,
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  root::{Root, RootProps},
  services::Services,
  startup::{Launch, WindowPosition},
  test_support::signed_in,
  theme::{self, Mode},
  ui::loader,
};
use orchester_workspace::sharing::cloud::BUILD_ADDRESS;
use signed_in::account_cloud::AccountCloud;
use tokio::runtime::Runtime;

/// The stand-in's port in a build whose own cloud it is.
const STAND_IN_PORT: u16 = 47990;

/// What the window opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
  /// A first run (`Lpznn`, `Rmtl1`).
  FirstRun,
  /// A first run, the MCP item's card on hover (`e2DPHR`).
  McpCard,
  /// *Advanced · cloud address* open with a test cloud set (`S8aPFV`).
  Advanced,
  /// A wrong email or password (`Atym1`).
  Wrong,
  /// No internet on a first run (`i8HD8`, `bHBBo`).
  Unreachable,
  /// *Confirm your email first* (`vovth`).
  Unconfirmed,
  /// *Too many attempts* (`t2C4a`).
  Paused,
  /// Signing in (`U0v7kx`, `G1ermB`).
  SigningIn,
  /// The gate came back: signed out from another device (`ztigw`, `i3jJp`).
  Revoked,
  /// The gate came back: the password changed (`obc19`).
  PasswordReset,
  /// The gate came back: the account was deleted (no frame).
  AccountDeleted,
  /// The gate came back with no reason this app knows (`obc19`).
  Other,
  /// After *Sign out and lock* (`mnT1S`).
  SignedOut,
  /// Signed in, Settings › Orchester asking before it signs out (`p2VeL`).
  SignOut,
  /// Signed in and offline: the status bar's item (`ZSB6h`).
  Offline,
  /// The same, its card on hover.
  OfflineCard,
  /// The code step after the password of an account with two-factor authentication (`Tmwkg`, `Dix2w`).
  Code,
  /// The code step with a recovery code (`hJ2yS`, `AZVn7`).
  Recovery,
  /// Verifying a code (`iflk6`, `MHk6K`).
  Verifying,
  /// A wrong code (`DtxLq`, `d0gfiq`).
  WrongCode,
  /// Too many codes: paused (`qZyaZ`, `LfTnj`).
  TooManyCodes,
  /// The challenge ended: back to the password (`R2wOmb`, `Z0ceYP`).
  TimedOut,
  /// Signed out to sign in again with a code, which accepting an invite to Billing needs (`Nq3Mf`, `y4WjRA`).
  SignInForCode,
}

impl State {
  const ALL: [(&'static str, Self); 23] = [
    ("first-run", Self::FirstRun),
    ("mcp-card", Self::McpCard),
    ("advanced", Self::Advanced),
    ("wrong", Self::Wrong),
    ("unreachable", Self::Unreachable),
    ("unconfirmed", Self::Unconfirmed),
    ("paused", Self::Paused),
    ("signing-in", Self::SigningIn),
    ("revoked", Self::Revoked),
    ("password-reset", Self::PasswordReset),
    ("account-deleted", Self::AccountDeleted),
    ("other", Self::Other),
    ("signed-out", Self::SignedOut),
    ("sign-out", Self::SignOut),
    ("offline", Self::Offline),
    ("offline-card", Self::OfflineCard),
    ("code", Self::Code),
    ("recovery", Self::Recovery),
    ("verifying", Self::Verifying),
    ("wrong-code", Self::WrongCode),
    ("too-many-codes", Self::TooManyCodes),
    ("timed-out", Self::TimedOut),
    ("sign-in-for-code", Self::SignInForCode),
  ];

  fn named(name: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .find(|(known, _)| *known == name)
      .map(|(_, state)| *state)
  }

  /// The reason the cloud gives when it ends the sign-in, for the states where it does.
  const fn reason(self) -> Option<Option<&'static str>> {
    match self {
      Self::Revoked => Some(Some("revoked")),
      Self::PasswordReset => Some(Some("password_reset")),
      Self::AccountDeleted => Some(Some("account_deleted")),
      Self::Other => Some(None),
      _ => None,
    }
  }

  /// Whether the state is past the password of an account with two-factor authentication.
  const fn code_step(self) -> bool {
    matches!(
      self,
      Self::Code | Self::Recovery | Self::Verifying | Self::WrongCode | Self::TooManyCodes | Self::TimedOut
    )
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let mut args = std::env::args().skip(1);
  let name = args.next().unwrap_or_else(|| "first-run".to_owned());
  let state = State::named(&name).ok_or_else(|| {
    let names: Vec<&str> = State::ALL.iter().map(|(name, _)| *name).collect();
    format!("expected one of {}", names.join(", "))
  })?;
  let (narrow, tabs) = options(args)?;
  let position = WindowPosition::from_environment()?;
  let scratch = qa_folder()?;
  std::fs::create_dir_all(&scratch)?;
  let data = tempfile::Builder::new().prefix("app-").tempdir_in(&scratch)?;
  let runtime = Runtime::new()?;
  let own = BUILD_ADDRESS == format!("http://localhost:{STAND_IN_PORT}");
  let cloud = if own {
    AccountCloud::start_on(STAND_IN_PORT)
  } else {
    AccountCloud::start()
  };
  let launch = signed_in::launch_signed_out(
    &data.path().join("orchester.db"),
    data.path().join("no-runner"),
    runtime.handle(),
  );
  let services = launch.services().ok_or("the QA session did not start")?.clone();
  if !own {
    let address = ServerAddress::parse(&cloud.url())?;
    runtime.block_on(services.cloud.set_address(Some(address)))?;
  }
  seed(state, (&services, &runtime), (&cloud, own))?;
  println!(
    "pid {}\nmcp {}\ndata {}\nstate {name}\nwidth {}\nown cloud {own}",
    std::process::id(),
    scratch.join("mcp").display(),
    data.path().display(),
    if narrow { 960 } else { 1440 }
  );
  window(&runtime, &launch, (state, narrow, position), (tabs, &cloud))?;
  launch.end();
  Ok(())
}

/// `--narrow` and `--tab <count>` after the state's name: 960 wide, and Tab pressed `count` times once the state shows.
fn options(mut args: impl Iterator<Item = String>) -> Result<(bool, usize), Box<dyn Error>> {
  let (mut narrow, mut tabs) = (false, 0);
  while let Some(arg) = args.next() {
    match arg.as_str() {
      "--narrow" => narrow = true,
      "--tab" => tabs = args.next().ok_or("--tab takes a count")?.parse()?,
      other => return Err(format!("unknown option {other}; expected --narrow or --tab <count>").into()),
    }
  }
  Ok((narrow, tabs))
}

/// What the person and the cloud did before the window opens on `state`.
fn seed(
  state: State,
  (services, runtime): (&Services, &Runtime),
  (cloud, own): (&AccountCloud, bool),
) -> Result<(), Box<dyn Error>> {
  let signed_in = matches!(
    state,
    State::Revoked
      | State::PasswordReset
      | State::AccountDeleted
      | State::Other
      | State::SignedOut
      | State::SignOut
      | State::Offline
      | State::OfflineCard
      | State::SignInForCode
  );
  // On a build whose own cloud the stand-in is, nothing is set in Advanced: the frames draw the default.
  if signed_in && own {
    let password = orchester_api::Credential::new(signed_in::account_cloud::PASSWORD.into());
    runtime.block_on(services.cloud.sign_in(signed_in::account_cloud::EMAIL.into(), password))?;
  } else if signed_in {
    signed_in::sign_in(services, runtime.handle(), cloud);
  }
  if let Some(reason) = state.reason() {
    cloud.end_token(reason, orchester_core::time::Timestamp::now().unix_millis());
    if runtime.block_on(services.cloud.check_sign_in()).is_ok() {
      return Err("the cloud still accepted the sign-in".into());
    }
  }
  if state.code_step() {
    cloud.require_codes();
  }
  match state {
    State::SignedOut => runtime.block_on(services.cloud.sign_out())?,
    State::SignInForCode => runtime.block_on(services.cloud.sign_out_for_code(Some("Billing".into())))?,
    State::Offline | State::OfflineCard => {
      cloud.set_down(true);
      if runtime.block_on(services.cloud.check_sign_in()).is_ok() {
        return Err("the cloud still answered".into());
      }
    }
    State::Advanced => {
      let address = ServerAddress::parse("https://cloud.test.lurq.dev")?;
      runtime.block_on(services.cloud.set_address(Some(address)))?;
    }
    State::Unreachable => cloud.set_down(true),
    State::SigningIn => cloud.slow_sign_in(std::time::Duration::from_secs(3600)),
    _ => {}
  }
  Ok(())
}

/// The window on `launch` until it closes, with the app's MCP server in QA mode, after [`drive`] took it to `state`.
fn window(
  runtime: &Runtime,
  launch: &Launch,
  (state, narrow, position): (State, bool, Option<WindowPosition>),
  (tabs, cloud): (usize, &AccountCloud),
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
      link: link.clone(),
      mcp: McpLink::new(&server),
      start: launch.start(),
    },
  );
  let width = if narrow { 960 } else { 1440 };
  tree.resize(width, 900);
  drive::to(&mut app, &mut tree, (state, &link, cloud), tabs)?;
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  let window = WinitWindow::new(app, tree)
    .with_title("Orchester sign-in gate QA")
    .with_icon(brand::window_icon()?)
    .with_size(width, 900)
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

/// `.tmp/qa/signin-gate/` of this checkout.
fn qa_folder() -> Result<PathBuf, &'static str> {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .ancestors()
    .nth(2)
    .map(|checkout| checkout.join(".tmp").join("qa").join("signin-gate"))
    .ok_or("the crate is two folders into the checkout")
}
