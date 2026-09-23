use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{
    ChromeBorderPolicy, ChromeTitleBar, Column, Text, WindowChrome, WindowChromeMode, WindowChromeProps, WindowControls,
  },
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};

use crate::{
  background,
  messages::{self, Message},
  navigation::{Navigation, Screen},
  screens::{project::ProjectShell, setup::ProjectSetup, unavailable, welcome::Welcome},
  theme::{ExtraColor, TITLE_BAR_HEIGHT},
  ui::{icon::Icon, message_text, panel::RecoveryPanel},
  workspace::Workspace,
};

/// How startup went, decided in `main` before the window opens.
#[derive(Clone, PartialEq)]
pub enum Startup {
  Ready(Workspace),
  /// The application database cannot be used; `path` is the file involved, if known.
  Failed {
    message: Message,
    path: Option<String>,
  },
}

impl lurq::app::component::DevtoolsInspectable for Startup {}

pub struct Root {
  screen: Signal<Option<Screen>>,
}

impl Component for Root {
  type Props = Startup;

  fn create(ctx: &mut Ctx) -> Self {
    let screen = ctx.signal(None);
    if let Startup::Ready(workspace) = ctx.props::<Startup>().clone() {
      ctx.provide(Navigation::new(workspace, screen.clone()));
    }
    Self { screen }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let content = match ctx.props::<Startup>().clone() {
      Startup::Failed { message, path } => startup_failed(ctx, &message, path.as_deref()),
      Startup::Ready(workspace) => self.workspace(ctx, workspace),
    };
    let screen = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .background(ExtraColor::Surface.role())
      .child(content);
    window_chrome(ctx, screen.into())
  }
}

impl Root {
  fn workspace(&self, ctx: &mut Ctx, workspace: Workspace) -> Element {
    let navigation = Navigation::new(workspace.clone(), self.screen.clone());
    let landing = ctx.future(workspace, |workspace| async move {
      background::run(move || workspace.landing())
        .await?
        .map(Screen::from)
        .map_err(|error| {
          tracing::error!(%error, "cannot read the projects of the workspace");
          messages::store_error(&error)
        })
    });
    let landing = landing.state().get();
    if let Some(error) = landing.error.as_ref() {
      return startup_failed(ctx, error, None);
    }
    match self.screen.get().or(landing.data) {
      // Reading the workspace takes milliseconds; nothing is shown until it is known.
      None => Column::new().into(),
      Some(Screen::Welcome) => ctx.mount_keyed::<Welcome>("welcome", ()),
      Some(Screen::Setup) => ctx.mount_keyed::<ProjectSetup>("setup", ()),
      Some(Screen::Project(project)) => ctx.mount_keyed::<ProjectShell>("project", project),
      Some(Screen::Unavailable {
        name,
        repo_root,
        reason,
      }) => unavailable::render(ctx, &navigation, &name, &repo_root, &reason),
    }
  }
}

/// Custom chrome (`QQmsI` on Windows, `V70DN` on macOS): no native title bar or border, a
/// canvas-colored bar whose empty part drags the window and maximizes on double-click.
/// The window controls are lurq's `WindowControls`; see the report on their styling gap.
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
        .controls(WindowControls::new()),
    )
    .content(screen)
    .mount(ctx)
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
