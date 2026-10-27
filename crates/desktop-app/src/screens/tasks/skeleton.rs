//! What stands in the rows' or the columns' place while the project's tasks are read (`05 · Loading` `R1nQlx` in
//! `Tasks · redesign` `Lw1Gb`, `17 · Loading` `Z1PMlo` in `Tasks board · redesign` `J6gQo`): after 300 ms, so a fast
//! reading never flashes it, bones in the geometry of what comes: the list's rows (a status dot, the key, the title
//! with a tag now and then, the state, the priority and the age, under group heads, the lower lines fading away), or
//! the board's columns with their real heads over card bones (`kanban::skeleton`). The header, the search and New task
//! work meanwhile; the views count with bones. It pulses gently, and stays still with reduced motion (`ui::skeleton`).
//! *Reading Kontur's tasks…* (`tasks.loading`) stays its name for tools.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{
  kanban::{self, display::Columns},
  row::ROW_HEIGHT,
};
use crate::{
  routes::TaskListTab,
  theme::{ExtraSpacing, SKELETON_DELAY, SKELETON_FADE, TasksSpacing},
  ui::skeleton::{bone, pulsing, round_bone},
};

/// A row's cells (`Row bone` `C8Z6Wd`): the status dot, the key, the actions' room, the priority and the age.
const STATUS: f32 = 14.0;
const KEY_CELL: f32 = 52.0;
const KEY: (f32, f32) = (44.0, 9.0);
const TITLE_HEIGHT: f32 = 11.0;
const TAG_HEIGHT: f32 = 9.0;
const STATE_HEIGHT: f32 = 10.0;
const ACTIONS_CELL: f32 = 150.0;
const PRIORITY_CELL: f32 = 16.0;
const PRIORITY: (f32, f32) = (12.0, 10.0);
const AGE_CELL: f32 = 30.0;
const AGE: (f32, f32) = (18.0, 9.0);
/// A group head (`Group bone` `irQNK`): 34 high, its chevron, label and count bones.
const GROUP_HEIGHT: f32 = 34.0;
const CHEVRON: f32 = 12.0;
const GROUP_BONE: f32 = 10.0;
const GROUP_COUNT: f32 = 14.0;

/// One line of the list's skeleton: a group head with its label's width, or a row with its title's, its tag's and its
/// state's (`R1nQlx`).
#[derive(Clone, Copy)]
enum Line {
  Group(f32),
  Row(f32, Option<f32>, Option<f32>),
}

/// The list skeleton's lines, as `R1nQlx` draws them.
const LINES: [Line; 19] = [
  Line::Group(64.0),
  Line::Row(340.0, Some(60.0), Some(120.0)),
  Line::Row(280.0, None, Some(96.0)),
  Line::Row(420.0, Some(72.0), Some(140.0)),
  Line::Row(220.0, Some(48.0), None),
  Line::Row(360.0, None, Some(120.0)),
  Line::Group(56.0),
  Line::Row(300.0, Some(56.0), Some(96.0)),
  Line::Row(250.0, None, Some(140.0)),
  Line::Row(390.0, Some(64.0), None),
  Line::Group(52.0),
  Line::Row(310.0, None, Some(120.0)),
  Line::Row(270.0, Some(52.0), Some(96.0)),
  Line::Row(350.0, None, Some(140.0)),
  Line::Group(84.0),
  Line::Row(230.0, Some(60.0), None),
  Line::Row(400.0, None, Some(120.0)),
  Line::Row(290.0, Some(48.0), Some(96.0)),
  Line::Row(330.0, None, Some(140.0)),
];

/// Where the skeleton stands in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  List,
  /// The board of a view, its columns by what Display chose.
  Board(TaskListTab, Columns),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonProps {
  pub place: Place,
  /// *Reading Kontur's tasks…*, its name for tools.
  pub label: std::sync::Arc<str>,
}

impl DevtoolsInspectable for SkeletonProps {}

pub struct Skeleton {
  shown: Signal<bool>,
  /// Shows the bones once the reading took [`SKELETON_DELAY`].
  _delay: Timeout,
}

impl Component for Skeleton {
  type Props = SkeletonProps;

  fn create(ctx: &mut Ctx) -> Self {
    let shown = ctx.signal(false);
    let show = shown.clone();
    let delay = ctx.create_timeout(SKELETON_DELAY, move || show.set(true));
    delay.start();
    Self { shown, _delay: delay }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SkeletonProps>().clone();
    let shown = self.shown.get();
    let frame = Column::new()
      .id("tasks-loading")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .describe("a11y_name", props.label.clone())
      .describe("state", if shown { "skeleton" } else { "waiting" });
    if !shown {
      return frame;
    }
    let bones = match props.place {
      Place::List => list(),
      Place::Board(tab, columns) => kanban::skeleton::board(ctx, (tab, columns)),
    };
    frame.child(pulsing(bones))
  }
}

/// The list's skeleton: its lines, 2 apart, each 28 from the panel's right edge as the rows are.
fn list() -> Column {
  let fading = LINES.len() - SKELETON_FADE.len();
  Column::new()
    .id("tasks-loading-rows")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().right(TasksSpacing::Sides.role()))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(LINES.iter().enumerate().map(|(index, line)| {
      let line = match *line {
        Line::Group(label) => group(label),
        Line::Row(title, tag, state) => row(title, tag, state),
      };
      match index.checked_sub(fading) {
        Some(fade) => line.opacity(SKELETON_FADE[fade]),
        None => line,
      }
    }))
}

/// A group head's bones: its chevron, label and count.
fn group(label: f32) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GROUP_HEIGHT))
    .padding(
      Padding::new()
        .top(TasksSpacing::GroupTop.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(bone(CHEVRON, CHEVRON))
    .child(bone(label, GROUP_BONE))
    .child(bone(GROUP_COUNT, GROUP_BONE))
}

/// A row's bones in the row's cells.
fn row(title: f32, tag: Option<f32>, state: Option<f32>) -> Row {
  let cell = |width: f32, justify: Justify| {
    Row::new()
      .width(Dimension::Px(width))
      .flex_shrink(0.0)
      .justify(justify)
      .align_items(Alignment::Center)
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(round_bone(STATUS))
    .child(cell(KEY_CELL, Justify::Start).child(bone(KEY.0, KEY.1)))
    .child(
      Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(bone(title, TITLE_HEIGHT))
        .with_children(tag.map(|tag| bone(tag, TAG_HEIGHT))),
    )
    .with_children(state.map(|state| bone(state, STATE_HEIGHT)))
    .child(cell(ACTIONS_CELL, Justify::End))
    .child(cell(PRIORITY_CELL, Justify::Center).child(bone(PRIORITY.0, PRIORITY.1)))
    .child(cell(AGE_CELL, Justify::End).child(bone(AGE.0, AGE.1)))
}
