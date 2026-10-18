//! The Details tab (`Task · Details` in `vShIt`: `UxsRH`, `R3Wkvd`, `ZSiHd`, and at 960 `Dxn8C`): what the
//! task asks for in full, its title, description and every acceptance criterion, in the version the
//! person picks among those Orchester kept (`requirements`), and beside it the setup of the task's latest
//! run apart from the task's current organization (`parameters`). The criteria are requirements: nothing
//! here says whether a run met them. In a narrow window the two side sections go under the requirements.
//!
//! The version shown is kept by the frame across tabs and readings of the task; one that a new reading no
//! longer offers (the task's latest run changed) gives way to the default (`TaskRequirements::shown`).

mod parameters;
mod requirements;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, Breakpoint, PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical},
  node::{Element, border::Border, dimension::Dimension},
};

use super::{PageState, TaskContext};
use crate::theme::TaskSpacing;

/// The side column's width (`j9Jzun`).
const SIDE_WIDTH: f32 = 340.0;

#[derive(Clone, PartialEq)]
pub struct DetailsProps {
  pub task: TaskContext,
}

impl lurq::app::component::DevtoolsInspectable for DetailsProps {}

pub struct TaskDetailsTab;

impl Component for TaskDetailsTab {
  type Props = DetailsProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DetailsProps>().clone();
    let state = PageState::of(ctx);
    let requirements = &props.task.detail.requirements;
    let chosen = state.requirements.get();
    let shown = requirements.shown(chosen);
    // Set only when it changes: what is shown is then what the picker holds.
    if chosen != Some(shown) {
      state.requirements.set(Some(shown));
    }
    let narrow = ctx.breakpoint() < Some(Breakpoint::Xl);
    let column = requirements::column(ctx, &props.task, (shown, &state.requirements), narrow);
    let sides = parameters::sections(ctx, &props.task);
    if narrow {
      return narrow_layout(column, sides, &state);
    }
    wide_layout(column, sides, &state)
  }
}

/// The requirements scrolling at the left and the side sections in their column at the right
/// (`Details body` `uhbXe`).
fn wide_layout(column: Element, sides: Vec<Element>, state: &PageState) -> Element {
  let side = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(TaskSpacing::Panel.role())
    .spacing(SpacingSize::Xl)
    .with_children(sides);
  Row::new()
    .id("task-details")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(
      Column::new().flex(1.0).height(Dimension::Pct(100.0)).child(
        ScrollVertical::new(column)
          .id("task-details-scroll")
          .with_scroll_state(state.details_scroll.clone())
          .width(Dimension::Pct(100.0))
          .height(Dimension::Pct(100.0)),
      ),
    )
    .child(
      Column::new()
        .id("task-details-side")
        .width(Dimension::Px(SIDE_WIDTH))
        .height(Dimension::Pct(100.0))
        .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(ScrollVertical::new(side).width(Dimension::Pct(100.0)).flex(1.0)),
    )
    .into()
}

/// The requirements with the side sections under them, side by side, all scrolling together (`nIRiC` in
/// `Dxn8C`).
fn narrow_layout(column: Element, sides: Vec<Element>, state: &PageState) -> Element {
  let sides = Row::new()
    .id("task-details-side")
    .width(Dimension::Pct(100.0))
    .padding(TaskSpacing::Panel.role())
    .spacing(SpacingSize::Xl)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(
      sides
        .into_iter()
        .map(|side| Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(side)),
    );
  let content = Column::new().width(Dimension::Pct(100.0)).child(column).child(sides);
  Column::new()
    .id("task-details")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(
      ScrollVertical::new(content)
        .id("task-details-scroll")
        .with_scroll_state(state.details_scroll.clone())
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0)),
    )
    .into()
}
