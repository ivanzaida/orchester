//! The window's root: the router inside the chrome, with the window's history and tab input around it.
//! It creates the tabs and the router once, and hands their navigation to the app's MCP tools.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Router,
  node::Element,
  router::RouterHandle,
};

use crate::{
  chrome,
  mcp::{McpStatus, WindowLink},
  navigation::{Navigation, input::window_input, tabs::TabSet},
  routes,
};

/// The element id of the window's content, whose size `app_state` reports.
pub const WINDOW_ID: &str = "app-window";

#[derive(Clone, PartialEq)]
pub struct RootProps {
  /// Where the root leaves the window's navigation for the app's MCP tools.
  pub link: WindowLink,
  /// Whether the app's MCP server listens, for the status bar.
  pub mcp: McpStatus,
}

impl DevtoolsInspectable for RootProps {}

pub struct Root {
  router: RouterHandle,
  navigation: Navigation,
}

impl Component for Root {
  type Props = RootProps;

  fn create(ctx: &mut Ctx) -> Self {
    let router = ctx.router(routes::routes());
    let navigation = Navigation::new(ctx.signal(TabSet::default()), router.clone());
    ctx.props::<RootProps>().link.attach(navigation.clone());
    Self { router, navigation }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Provided on every render: a render from the root replaces the inherited context.
    let mcp = ctx.props::<RootProps>().mcp;
    ctx.provide(mcp);
    ctx.provide(self.navigation.clone());
    let screen = Router::mount(ctx, self.router.clone());
    let window = chrome::chrome(ctx, screen);
    window_input(&self.navigation, window).id(WINDOW_ID)
  }
}
