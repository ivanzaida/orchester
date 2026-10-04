use lurq::{
  app::{component::Component, ctx::Ctx, theme::TypographyStyle},
  components::{Column, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;
use orchester_ipc::SessionEnd;

use crate::{
  activity::Activity,
  background,
  close::Lifecycle,
  embedded_mcp::McpStatus,
  messages::{self, Message},
  navigation::Places,
  pipelines::zoom::SessionZooms,
  programs::Programs,
  screens::status_bar::{StatusBar, StatusBarProps},
  secrets::Vault,
  sharing::{
    Servers,
    feed::{ServerFeed, ServerFeedLoader},
  },
  tabs::{
    feed::{WindowFeed, WindowFeedLoader},
    restore::{self, Opening},
    window::{LEADING_WIDTH, Window, WindowProps, chrome, leading},
  },
  theme::ExtraColor,
  ui::{icon::Icon, message_text, panel::RecoveryPanel},
  workspace::Workspace,
};

/// How startup went, decided in `main` before the window opens.
#[derive(Clone, PartialEq)]
pub enum Startup {
  Ready {
    workspace: Workspace,
    lifecycle: Lifecycle,
    /// Background operations of the session, for the status bar.
    activity: Activity,
    /// How `gh` runs for pull request sync, and git for the recovery's worktree states.
    programs: Programs,
    /// Where secret values are kept: the OS keychain.
    vault: Vault,
    /// How the previous session ended: after Quit now or a crash, the start-up recovery asks
    /// about the runs it left interrupted (02.E `o8DCd`).
    previous_end: SessionEnd,
    /// When this session started.
    session_start: Timestamp,
  },
  /// The application database cannot be used; `path` is the file involved, if known.
  Failed { message: Message, path: Option<String> },
}

impl lurq::app::component::DevtoolsInspectable for Startup {}

/// What `main` hands the window: how startup went and whether the embedded MCP server listens.
#[derive(Clone, PartialEq)]
pub struct RootProps {
  pub startup: Startup,
  pub mcp: McpStatus,
}

impl lurq::app::component::DevtoolsInspectable for RootProps {}

pub struct Root {
  /// What routes stand for, shared by every tab: present once the session started.
  places: Option<Places>,
}

impl Component for Root {
  type Props = RootProps;

  fn create(ctx: &mut Ctx) -> Self {
    let RootProps { startup, mcp } = ctx.props::<RootProps>().clone();
    ctx.provide(mcp);
    // Without a session nothing reports activity; the status bar still shows the version.
    let activity = match &startup {
      Startup::Ready { activity, .. } => activity.clone(),
      Startup::Failed { .. } => Activity::new(),
    };
    ctx.provide(WindowFeed::new(&activity));
    ctx.provide(activity);
    let Startup::Ready {
      workspace,
      lifecycle,
      programs,
      vault,
      ..
    } = startup
    else {
      return Self { places: None };
    };
    ctx.provide(ServerFeed::new(Servers::new(workspace, vault.clone())));
    ctx.provide(programs);
    ctx.provide(vault);
    ctx.provide(SessionZooms::default());
    let close = lifecycle.clone();
    ctx
      .window()
      .on_close_requested(move |request| close.on_close_requested(request));
    ctx.provide(lifecycle.runners().clone());
    ctx.provide(lifecycle);
    Self {
      places: Some(Places::default()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let feed = ctx.mount_keyed::<WindowFeedLoader>("window-feed", ());
    let servers = self
      .places
      .is_some()
      .then(|| ctx.mount_keyed::<ServerFeedLoader>("server-feed", ()));
    let window = match ctx.props::<RootProps>().startup.clone() {
      Startup::Failed { message, path } => {
        let screen = with_status_bar(ctx, startup_failed(ctx, &message, path.as_deref()));
        bare_window(ctx, screen)
      }
      Startup::Ready {
        workspace,
        lifecycle,
        programs,
        previous_end,
        session_start,
        ..
      } => {
        let places = self.places.clone().expect("create keeps the places of a ready startup");
        self.window(
          ctx,
          places,
          (workspace, lifecycle, programs),
          (previous_end, session_start),
        )
      }
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(feed)
      .with_children(servers)
      .child(window)
  }
}

impl Root {
  /// The window with its tabs, once they are read; until then the bare chrome, for the few
  /// milliseconds reading takes.
  fn window(
    &self,
    ctx: &mut Ctx,
    places: Places,
    (workspace, lifecycle, programs): (Workspace, Lifecycle, Programs),
    (previous_end, session_start): (SessionEnd, Timestamp),
  ) -> Element {
    let opening = ctx.future((workspace.clone(), places.clone()), |(workspace, places)| async move {
      background::run(move || restore::opening(&workspace, &places))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the projects of the workspace");
          messages::store_error(&error)
        })
    });
    let opening: Option<Opening> = match opening.state().get() {
      state if state.error.is_some() => {
        let error = state.error.expect("checked above");
        let screen = with_status_bar(ctx, startup_failed(ctx, &error, None));
        return bare_window(ctx, screen);
      }
      state => state.data,
    };
    let Some(opening) = opening else {
      return bare_window(ctx, Column::new().into());
    };
    let props = WindowProps {
      opening,
      workspace,
      places,
      lifecycle,
      programs,
      previous_end,
      session_start,
    };
    ctx.mount_keyed::<Window>("window", props)
  }
}

/// The chrome around `screen` without tabs: before the session's tabs are known, or when it did
/// not start.
fn bare_window(ctx: &mut Ctx, screen: Element) -> Element {
  let screen = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(ExtraColor::Surface.role())
    .child(screen);
  let title_bar = leading(ctx, LEADING_WIDTH);
  chrome(ctx, title_bar, screen.into())
}

/// A screen without a project above the status bar (`pmOpt`), which then shows only the
/// activity and the version.
pub(crate) fn with_status_bar(ctx: &mut Ctx, screen: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(Column::new().width(Dimension::Pct(100.0)).flex(1.0).child(screen))
    .child(ctx.mount_keyed::<StatusBar>("status-bar", StatusBarProps { project: None }))
    .into()
}

/// The application database could not be opened or read. There is no recovery design yet, so
/// this only says what happened and where.
fn startup_failed(ctx: &Ctx, message: &Message, path: Option<&str>) -> Element {
  let mut panel = RecoveryPanel::new(
    Icon::AlertCircle,
    &ctx.t("recovery.startupFailed.title"),
    &message_text(ctx, message),
  );
  if let Some(path) = path {
    panel = panel.detail(Text::new(path).variant(TypographyStyle::Caption).selectable(true));
  }
  centered(panel.into())
}

pub fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}
