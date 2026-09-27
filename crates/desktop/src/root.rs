use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{
    ChromeBorderPolicy, ChromeTitleBar, Column, Router, Text, WindowChrome, WindowChromeMode, WindowChromeProps,
    WindowControlKind, WindowControls,
  },
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;
use orchester_ipc::SessionEnd;

use crate::{
  activity::Activity,
  background,
  close::Lifecycle,
  messages::{self, Message},
  navigation::{self, Navigation, Places, Start, with_history_input},
  programs::Programs,
  screens::{
    close_dialog::CloseConfirmation,
    startup_recovery::{StartupRecovery, StartupRecoveryProps},
    status_bar::{StatusBar, StatusBarProps},
  },
  secrets::Vault,
  theme::{ExtraColor, TITLE_BAR_HEIGHT, WINDOW_CONTROL_WIDTH},
  ui::{
    icon::{Icon, window_control},
    message_text,
    panel::RecoveryPanel,
  },
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

pub struct Root {
  /// Present once the session started: a failed start has no screens to go between.
  navigation: Option<Navigation>,
}

impl Component for Root {
  type Props = Startup;

  fn create(ctx: &mut Ctx) -> Self {
    let startup = ctx.props::<Startup>().clone();
    // Without a session nothing reports activity; the status bar still shows the version.
    let activity = match &startup {
      Startup::Ready { activity, .. } => activity.clone(),
      Startup::Failed { .. } => Activity::new(),
    };
    ctx.provide(activity);
    let Startup::Ready {
      workspace,
      lifecycle,
      programs,
      vault,
      ..
    } = startup
    else {
      return Self { navigation: None };
    };
    let places = Places::default();
    let router = ctx.router(navigation::routes(&places));
    let navigation = Navigation::new(workspace, router, places);
    ctx.provide(programs);
    ctx.provide(vault);
    ctx.provide(navigation.clone());
    let close = lifecycle.clone();
    ctx
      .window()
      .on_close_requested(move |request| close.on_close_requested(request));
    ctx.provide(lifecycle.runners().clone());
    ctx.provide(lifecycle);
    Self {
      navigation: Some(navigation),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let content = match ctx.props::<Startup>().clone() {
      Startup::Failed { message, path } => with_status_bar(ctx, startup_failed(ctx, &message, path.as_deref())),
      Startup::Ready {
        workspace,
        lifecycle,
        programs,
        previous_end,
        session_start,
        ..
      } => {
        let navigation = self
          .navigation
          .as_ref()
          .expect("create sets up the navigation of a ready startup");
        Self::workspace(
          ctx,
          navigation,
          (workspace, &programs, (previous_end, session_start)),
          &lifecycle,
        )
      }
    };
    let screen = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .background(ExtraColor::Surface.role())
      .child(content);
    let window = window_chrome(ctx, screen.into());
    match &self.navigation {
      Some(navigation) => with_history_input(navigation.router(), window),
      None => window,
    }
  }
}

impl Root {
  fn workspace(
    ctx: &mut Ctx,
    navigation: &Navigation,
    (workspace, programs, (previous_end, session_start)): (Workspace, &Programs, (SessionEnd, Timestamp)),
    lifecycle: &Lifecycle,
  ) -> Element {
    let landing = ctx.future(workspace.clone(), |workspace| async move {
      background::run(move || workspace.landing())
        .await?
        .map(Start::from)
        .map_err(|error| {
          tracing::error!(%error, "cannot read the projects of the workspace");
          messages::store_error(&error)
        })
    });
    let landing = landing.state().get();
    if let Some(error) = landing.error.as_ref() {
      return startup_failed(ctx, error, None);
    }
    // Reading the workspace takes milliseconds; the start route shows nothing until it is known.
    let shown_project = match &landing.data {
      Some(Start::Project(project)) => Some(project.project.id()),
      _ => None,
    };
    if let Some(start) = landing.data {
      navigation.arrive(start);
    }
    let recovery = StartupRecoveryProps {
      workspace,
      programs: programs.clone(),
      previous: previous_end,
      session_start,
      shown_project,
    };
    // The confirmation is a modal, so it takes no space in this column.
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(Router::mount(ctx, navigation.router().clone()))
      .child(ctx.mount_keyed::<CloseConfirmation>("close-confirmation", lifecycle.clone()))
      .child(ctx.mount_keyed::<StartupRecovery>("startup-recovery", recovery))
      .into()
  }
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

/// Custom chrome (`QQmsI` on Windows, `V70DN` on macOS): no native title bar or border, a
/// canvas-colored bar whose empty part drags the window and maximizes on double-click.
fn window_chrome(ctx: &mut Ctx, screen: Element) -> Element {
  WindowChrome::new()
    .props(
      WindowChromeProps::new()
        .mode(WindowChromeMode::CustomDesktop)
        .border(ChromeBorderPolicy::Hidden)
        .windows_height(TITLE_BAR_HEIGHT)
        .macos_height(TITLE_BAR_HEIGHT),
    )
    .title_bar(
      ChromeTitleBar::new()
        .background(PaletteColor::SurfaceBase)
        .border_bottom(None)
        .controls(window_controls()),
    )
    .content(screen)
    .mount(ctx)
}

/// Windows controls as in `QQmsI`: lucide icons in the muted text color and one shared hover
/// background; macOS keeps lurq's standard traffic lights (`V70DN`).
fn window_controls() -> WindowControls {
  WindowControls::new()
    .button_size(WINDOW_CONTROL_WIDTH, TITLE_BAR_HEIGHT)
    .content(WindowControlKind::Minimize, window_control(Icon::Minus))
    .content(WindowControlKind::Maximize, window_control(Icon::Square))
    .content(WindowControlKind::Restore, window_control(Icon::Copy))
    .content(WindowControlKind::Close, window_control(Icon::X))
    .foreground(PaletteColor::TextSecondary)
    .background(PaletteColor::SurfaceBase)
    .hover_background(PaletteColor::AccentMuted)
    .active_background(PaletteColor::AccentMuted)
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
