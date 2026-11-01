//! The window of the team scenarios a person's press reaches ([`team::drive`], [`team_clone`]): the app's root and MCP
//! server as `window::open` starts them, in an order of its own, because the press needs the tree before the window
//! runs; for `team-clone` the root has the stand-in clones in its context. Each other QA state opens the app's own
//! window.

use std::{
  error::Error,
  sync::{Arc, OnceLock},
};

use lurq::{
  app::{
    App, Tree, WindowCornerRadius,
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    wgpu_render::WgpuRenderEngine,
    winit_shell::WinitWindow,
  },
  node::Element,
};
use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  brand, i18n,
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  root::{Root, RootProps},
  startup::{Launch, WindowPosition},
  theme::{self, Mode},
  ui::{loader, skeleton},
};
use orchester_workspace::sharing::project::clone::Clones;
use tokio::runtime::Runtime;

use super::{team, team_clone::StandIn};

/// Takes the window of `launch` to `state` on `server`, with `clones` for `team-clone`, shows it, and returns once it
/// closed and its MCP server stopped.
pub fn open(
  runtime: &Runtime,
  launch: &Launch,
  position: Option<WindowPosition>,
  (state, server, clones): (team::State, &TeamServer, Option<&StandIn>),
) -> Result<(), Box<dyn Error>> {
  let appearance = Appearance::new(
    ThemeSetting::Dark,
    Arc::new(OnceLock::new()),
    launch.workspace().cloned(),
  );
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app)?;
  let mut tree = Tree::new();
  loader::install(&mut tree);
  skeleton::install(&mut tree);
  let link = WindowLink::default();
  link.attach_appearance(appearance.clone());
  let settings = McpSettings::of_build().with_environment();
  let mode = McpMode::of_build(launch.workspace()).ok_or("a QA copy has an MCP server")?;
  let config = mcp::config(settings, &link, &appearance, launch.services(), app.i18n());
  let mcp_server = AppMcp::start(
    &mut tree,
    runtime.handle(),
    mode,
    (config, settings),
    &link.input_gate(),
  );
  let root = RootProps {
    link: link.clone(),
    mcp: McpLink::new(&mcp_server),
    start: launch.start(),
  };
  match clones {
    Some(stand_in) => tree.mount_root::<Host>(
      &mut app,
      HostProps {
        root,
        clones: stand_in.clones.clone(),
      },
    ),
    None => tree.mount_root::<Root>(&mut app, root),
  }
  tree.resize(1440, 900);
  tree.pass_headless(&mut app);
  let driven = match (clones, launch.services()) {
    (Some(stand_in), Some(services)) => stand_in.drive((&mut app, &mut tree), &link, services),
    (Some(_), None) => Err("team-clone needs a started session".into()),
    (None, _) => team::drive::to(&mut app, &mut tree, (state, &link, server)),
  };
  let shown = driven.and_then(|()| show(app, tree, position));
  // Also on an error: `tools/qa-app.ps1` takes a discovery file left behind for a copy that started.
  mcp_server.shutdown();
  shown
}

#[derive(Clone)]
struct HostProps {
  root: RootProps,
  clones: Clones,
}

/// The stand-in clones never change while the window is open.
impl PartialEq for HostProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
  }
}

impl DevtoolsInspectable for HostProps {}

/// The app's root with the stand-in clones in its context, which *Link a folder* takes before the session's.
struct Host;

impl Component for Host {
  type Props = HostProps;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HostProps>().clone();
    ctx.provide(props.clones);
    ctx.mount_keyed::<Root>("app", props.root)
  }
}

/// Shows the window of `app` and `tree`, and returns once it closed.
fn show(app: App, mut tree: Tree, position: Option<WindowPosition>) -> Result<(), Box<dyn Error>> {
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  let title = app.i18n().t("shell.app.name");
  let window = WinitWindow::new(app, tree)
    .with_title(&title)
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
  Ok(())
}
