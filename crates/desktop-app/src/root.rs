//! The window's root: a started session's window, or the startup screen that says why there is none. A
//! session's window is the router inside the chrome, with the window's history and tab input around it
//! and the projects its tabs show; it creates the tabs, the router and the projects once, and hands them
//! to the app's MCP tools. The window's close requests go to the session's close flow, whose panel takes
//! the window's body while a close is held (`lifecycle`).
//!
//! Until this computer is signed in to Orchester Cloud, the session's window is the sign-in gate alone
//! (`screens::gate`): no tabs, menu, sidebar, screens or keyboard shortcuts. The tabs stay as they were behind it, so
//! signing in again opens them again; another account gets its own tabs (`navigation::accounts`). The projects'
//! runners keep running, and the close flow still holds a close.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Router, Stack},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
  router::RouterHandle,
};
use orchester_workspace::{
  orchestrator::recover,
  sharing::cloud::{Cloud, SignIn},
};

use crate::{
  browser::{AutoOpen, BrowserLink},
  chrome, hang_reports, lifecycle,
  mcp::{self, McpLink, WindowLink},
  navigation::{
    Navigation,
    accounts::AccountTabs,
    input::{FirstKeys, window_input},
    tabs::TabSet,
  },
  notifications::{Chosen, NotificationPrefs, NotificationsHost},
  open_link::{self, LinkHost},
  project::{Landing, Projects, ProjectsHost},
  routes,
  screens::gate::{GateProps, SignInGate},
  services::Services,
  shell::{needs_you::PanelOpen, status_bar, update::UpdateAsk},
  startup::{self, Problem},
  ui::chat::links::ChatLinks,
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
    if let Some(appearance) = link.appearance() {
      ctx.provide(appearance);
    }
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
      Start::Problem(problem) => {
        let beat = hang_reports::beat::beat(ctx, &link);
        Column::new()
          .id(WINDOW_ID)
          .width(Dimension::Pct(100.0))
          .height(Dimension::Pct(100.0))
          .with_children(beat)
          .child(startup::window(ctx, problem))
          .into()
      }
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
  needs_you: PanelOpen,
  /// Whether *Restart to update* asks about the runs at work, and a press refused in a state set for testing.
  update_ask: UpdateAsk,
  /// The closing panel's choice the keyboard moved to.
  focus: Signal<Option<lifecycle::Choice>>,
  first_keys: FirstKeys,
  /// The one handler every Markdown of the window follows its links with.
  links: ChatLinks,
  /// Whose tabs the window shows.
  account_tabs: AccountTabs,
  /// The person's notification settings, read by the notifications' host and changed in Settings.
  notifications: NotificationPrefs,
  /// The Browser window, which the window opens and points.
  browser: BrowserLink,
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
    let projects = Projects::new(landing.project, services.jobs.clone());
    link.attach(navigation.clone(), projects.clone());
    lifecycle::hook(ctx, &services.close);
    // The Orchestrator's decisions an earlier session left under way are settled by what happened, in its words.
    services.runtime.spawn(recover::recover_all(
      services.workspace.clone(),
      (services.session_start, mcp::domain::translate(ctx.i18n().clone())),
      services.runners.clone(),
    ));
    let browser = BrowserLink::new(
      services.clone(),
      ctx.window_opener(),
      navigation.clone(),
      ctx.t("browser.window.titleAlone"),
    );
    Self {
      router,
      account_tabs: AccountTabs::new(services.cloud.sign_in_state().account()),
      links: ChatLinks::new(navigation.clone()),
      navigation,
      projects,
      needs_you: PanelOpen::new(ctx.signal(false)),
      update_ask: UpdateAsk::new(ctx.signal(false), ctx.signal(None)),
      focus: ctx.signal(None),
      first_keys: FirstKeys::default(),
      notifications: NotificationPrefs::new(ctx.signal(Chosen::default())),
      browser,
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
    ctx.provide(self.links.clone());
    ctx.provide(self.projects.clone());
    ctx.provide(self.first_keys.clone());
    ctx.provide(self.needs_you.clone());
    ctx.provide(self.update_ask.clone());
    ctx.provide(self.notifications.clone());
    ctx.provide(self.browser.clone());
    let sign_in = follow_sign_in(ctx, &services.cloud);
    if let Some(account) = sign_in.account() {
      self.account_tabs.follow(account, &self.navigation, &self.projects);
    }
    let phase = lifecycle::phase(ctx, &services.close);
    let closing = phase.holds_window();
    let projects = ctx.mount_keyed::<ProjectsHost>("projects", ());
    let link = ctx.props::<SessionProps>().link.clone();
    let beat = hang_reports::beat::beat(ctx, &link);
    // Behind the sign-in gate nothing of the projects is told, on the screen or in a notification, and no page
    // opens by itself.
    let notifications = sign_in
      .is_signed_in()
      .then(|| ctx.mount_keyed::<NotificationsHost>("notifications", ()));
    let browser = sign_in
      .is_signed_in()
      .then(|| ctx.mount_keyed::<AutoOpen>("browser-auto-open", self.browser.clone()));
    let screen = match &sign_in {
      SignIn::SignedIn { .. } => Router::mount(ctx, self.router.clone()),
      SignIn::SignedOut { since, .. } => gate_screen(ctx, &sign_in, *since),
    };
    let overlay = lifecycle::overlay(ctx, phase, &services.close, &self.focus);
    // Over the page or the gate: what an `orchester://` link says, behind the gate as well.
    let links = ctx.mount_keyed::<LinkHost>(
      "open-links",
      open_link::host::Props {
        signed_in: sign_in.is_signed_in(),
      },
    );
    // The closing panel lies over the page, which stays as it was under it.
    let body = Stack::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(screen)
      .child(links)
      .with_children(overlay);
    let signed_in = sign_in.is_signed_in();
    let framed = if signed_in {
      chrome::chrome(ctx, body.into(), closing)
    } else {
      chrome::gate(ctx, body.into(), closing)
    };
    let window = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(projects)
      .with_children(beat)
      .with_children(notifications)
      .with_children(browser)
      .child(framed);
    // Behind the gate no shortcut opens a tab or a screen.
    window_input(
      &self.navigation,
      window.into(),
      (signed_in && !closing, self.first_keys.clone()),
    )
    .id(WINDOW_ID)
    .on_key_down(lifecycle::keys(&services.close, &self.focus))
  }
}

/// The gate in the content panel over its status bar (`Body` and `Status bar` of `Lpznn`).
fn gate_screen(ctx: &mut Ctx, sign_in: &SignIn, since: orchester_core::time::Timestamp) -> Element {
  let gate = ctx.mount_keyed::<SignInGate>(
    "sign-in-gate",
    GateProps {
      sign_in: sign_in.clone(),
    },
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceBase)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .padding(Padding::horizontal(SpacingSize::Sm))
        .child(gate),
    )
    .child(status_bar::gate(ctx, since))
    .into()
}

/// Whether this computer is signed in to Orchester Cloud, whenever that changes.
fn follow_sign_in(ctx: &mut Ctx, cloud: &Cloud) -> SignIn {
  ctx
    .stream(cloud.clone(), |cloud, emitter: StreamEmitter<SignIn, ()>| async move {
      let mut changes = cloud.follow_sign_in();
      loop {
        let sign_in = changes.borrow_and_update().clone();
        if !emitter.emit(sign_in) || changes.changed().await.is_err() {
          return;
        }
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_else(|| cloud.sign_in_state())
}
