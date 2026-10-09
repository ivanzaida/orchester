//! The startup screens in the window: the content panel with the screen in its middle, between the
//! title bar (the app menu and the window controls, no tabs) and a status bar with only the version.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::Column,
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{Problem, StartupProblem, already_open, failure};
use crate::{
  chrome,
  shell::status_bar,
  theme::{ExtraColor, StartupSpacing},
};

/// The width of a startup screen (`Startup · Orchester is already open` `g0Kj1`).
const SCREEN_WIDTH: f32 = 600.0;

/// The window of a session that did not start: `problem` in the content panel.
pub fn window(ctx: &mut Ctx, problem: Problem) -> Element {
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(Padding::horizontal(SpacingSize::Sm))
    .child(ctx.mount_keyed::<StartupScreen>("startup", problem));
  let window = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceBase)
    .child(body)
    .child(status_bar::version_only(ctx));
  chrome::bare(ctx, window.into())
}

/// One startup screen in the content panel (`Content · Startup` `RqhE7`).
pub struct StartupScreen {
  /// *Switch to that window* got no answer: the screen says so until it is pressed again.
  no_answer: Signal<bool>,
}

impl Component for StartupScreen {
  type Props = Problem;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      no_answer: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Problem { problem, log } = ctx.props::<Problem>().clone();
    let screen = match &problem {
      StartupProblem::AlreadyOpen { folder, holder } => already_open::screen(ctx, folder, *holder, &self.no_answer),
      StartupProblem::Failed(failure) => failure::screen(ctx, failure, log.as_deref()),
    };
    Column::new()
      .id("screen-startup")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(StartupSpacing::Panel.role())
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .background(PaletteColor::SurfacePanel)
      .rounded(RadiusSize::Lg)
      .border_inside(BorderSize::Sm, ExtraColor::PanelBorder.role())
      .clip()
      .child(screen)
  }
}

/// The column of one startup screen, 600 wide, its parts 20 apart.
pub(super) fn screen_column(id: &'static str) -> Column {
  Column::new()
    .id(id)
    .width(Dimension::Px(SCREEN_WIDTH))
    .spacing(StartupSpacing::Block.role())
}
