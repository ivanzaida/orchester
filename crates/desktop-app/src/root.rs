//! The window's root: a started session's window, or the startup screen that says why there is none. A
//! session's window is the router inside the chrome, with the window's history and tab input around it
//! and the projects its tabs show; it creates the tabs, the router and the projects once, and hands them
//! to the app's MCP tools. The window's close requests go to the session's close flow, whose panel takes
//! the window's body while a close is held (`lifecycle`).

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, Router, Stack},
  core::Signal,
  node::{Element, dimension::Dimension},
  router::RouterHandle,
};

use crate::{
  chrome, lifecycle,
  mcp::{McpLink, WindowLink},
  navigation::{
    Navigation,
    input::{FirstKeys, window_input},
    tabs::TabSet,
  },
  project::{Landing, Projects, ProjectsHost},
  routes,
  services::Services,
  startup::{self, Problem},
};

/// The element id of the window's content, whose size `app_state` reports.
pub const WINDOW_ID: &str = "app-window";

/// How startup went, decided before the window opens.
#[derive(Clone, PartialEq)]
pub enum Start {
  /// The session started.
  Ready(Box<Ready>),
  /// The window says why there is no session.
  Problem(Problem),
}

impl DevtoolsInspectable for Start {}

/// A started session: its services, and where the window opens.
#[derive(Clone, PartialEq)]
pub struct Ready {
  pub services: Services,
  pub landing: Landing,
}

#[derive(Clone, PartialEq)]
pub struct RootProps {
  /// Where the root leaves the window's navigation and projects for the app's MCP tools.
  pub link: WindowLink,
  /// The app's MCP server, for the status bar and Settings.
  pub mcp: McpLink,
  pub start: Start,
}

impl DevtoolsInspectable for RootProps {}

pub struct Root;

impl Component for Root {
  type Props = RootProps;

  fn create(ctx: &mut Ctx) -> Self {
    let RootProps { link, start, .. } = ctx.props::<RootProps>();
    if let Start::Problem(problem) = start {
      link.attach_startup(problem.problem.name());
    }
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RootProps { link, mcp, start } = ctx.props::<RootProps>().clone();
    // Provided on every render: a render from the root replaces the inherited context.
    ctx.provide(mcp);
    match start {
      Start::Ready(ready) => {
        let Ready { services, landing } = *ready;
        ctx.mount_keyed::<SessionWindow>(
          "session",
          SessionProps {
            link,
            services,
            landing,
          },
        )
      }
      Start::Problem(problem) => Column::new()
        .id(WINDOW_ID)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0))
        .child(startup::window(ctx, problem))
        .into(),
    }
  }
}

#[derive(Clone, PartialEq)]
struct SessionProps {
  link: WindowLink,
  services: Services,
  landing: Landing,
}

impl DevtoolsInspectable for SessionProps {}

/// The window of a started session, opened where `landing` says.
struct SessionWindow {
  router: RouterHandle,
  navigation: Navigation,
  projects: Projects,
  /// The closing panel's choice the keyboard moved to.
  focus: Signal<Option<lifecycle::Choice>>,
  first_keys: FirstKeys,
}

impl Component for SessionWindow {
  type Props = SessionProps;

  fn create(ctx: &mut Ctx) -> Self {
    let SessionProps {
      link,
      landing,
      services,
    } = ctx.props::<SessionProps>().clone();
    let router = ctx.router(routes::routes());
    let navigation = Navigation::new(ctx.signal(TabSet::at(landing.route)), router.clone());
    let projects = Projects::new(landing.project, services.jobs);
    link.attach(navigation.clone(), projects.clone());
    lifecycle::hook(ctx, &services.close);
    Self {
      router,
      navigation,
      projects,
      focus: ctx.signal(None),
      first_keys: FirstKeys::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let services = ctx.props::<SessionProps>().services.clone();
    let gate = ctx.props::<SessionProps>().link.input_gate();
    // Provided on every render: a render from the root replaces the inherited context.
    ctx.provide(services.clone());
    gate.bind_permissions(services.workspace.clone());
    ctx.provide(gate);
    ctx.provide(self.navigation.clone());
    ctx.provide(self.projects.clone());
    ctx.provide(self.first_keys.clone());
    let phase = lifecycle::phase(ctx, &services.close);
    let closing = phase.holds_window();
    let projects = ctx.mount_keyed::<ProjectsHost>("projects", ());
    let screen = Router::mount(ctx, self.router.clone());
    let overlay = lifecycle::overlay(ctx, phase, &services.close, &self.focus);
    // The closing panel lies over the page, which stays as it was under it.
    let body = Stack::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(screen)
      .with_children(overlay);
    let window = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(projects)
      .child(chrome::chrome(ctx, body.into(), closing));
    window_input(&self.navigation, window.into(), (!closing, self.first_keys.clone()))
      .id(WINDOW_ID)
      .on_key_down(lifecycle::keys(&services.close, &self.focus))
  }
}
