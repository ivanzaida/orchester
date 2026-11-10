//! A row of the flow: the rail (a line with a node on it) and the cell beside it (`Step 01` `k8rJ39`, `Gate · approval`
//! `ImXS2`, `Start` `a6LHRb` in `Steps` `uSoll`). The lines of the rows' rails meet, so the rails read as one line from
//! *ON TASK START* down to the end.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraRadius, FlowText, IconSize},
  ui::icon::{Icon, icon},
};

/// A node's width and height (`Node` `YsSsg`).
const NODE: f32 = 22.0;
/// The end's dot (`Dot` `JIZ3R`).
const DOT: f32 = 7.0;
/// What a step's node sits below, so it stands level with its card's first line (`Top` `CoWRr`), and a parallel
/// group's (`Top` `Y8J3s1`).
const STEP_TOP: f32 = 5.0;
const GROUP_TOP: f32 = 6.0;

/// What a node of the rail shows, and the line above it.
pub(super) enum Node {
  /// *ON TASK START*: a `play` glyph; nothing above it.
  Start,
  /// A step's number in a node of its own, level with its card's first line.
  Step(usize),
  /// A step that cannot run as it is: its node's outline in `warning` (`Node` `nNbyv` in P8).
  NotReady(usize),
  /// An approval gate on the line (`Gate` `V2URU`).
  Gate,
  /// A group of steps that run side by side (`Node` `ntPkr`).
  Parallel,
  /// The end of the line: a dot (`Node` `H2joy6`); nothing below it.
  End,
  /// Only the line, for what stands between the start and the end without being a step: the note that there are none,
  /// or that they cannot be drawn.
  Line,
}

impl Node {
  const fn top(&self) -> f32 {
    match self {
      Self::Step(_) | Self::NotReady(_) => STEP_TOP,
      Self::Parallel => GROUP_TOP,
      Self::Start | Self::Gate | Self::End | Self::Line => 0.0,
    }
  }

  const fn continues(&self) -> bool {
    !matches!(self, Self::End)
  }
}

/// One row of the flow: `node` on the rail and `cell` beside it. `room` is what the cell keeps under its content.
pub(super) fn row(ctx: &Ctx, id: &str, node: &Node, (cell, room): (Element, SpacingSize)) -> Element {
  let width = ctx.theme().border_size_value(BorderSize::Sm);
  let line = |id: String| {
    Row::new()
      .id(id)
      .width(Dimension::Px(width))
      .background(PaletteColor::BorderStrong)
  };
  let mut rail = Column::new()
    .id(format!("{id}-rail"))
    .width(Dimension::Px(NODE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center);
  if node.top() > 0.0 {
    rail = rail.child(line(format!("{id}-above")).height(Dimension::Px(node.top())));
  }
  rail = rail.with_children(mark(id, node));
  if node.continues() {
    rail = rail.child(line(format!("{id}-below")).flex(1.0));
  }
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Stretch)
    .child(rail)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .padding(Padding::new().bottom(room))
        .child(cell),
    )
    .into()
}

fn mark(id: &str, node: &Node) -> Option<Row> {
  let round = || {
    Row::new()
      .id(format!("{id}-node"))
      .width(Dimension::Px(NODE))
      .height(Dimension::Px(NODE))
      .flex_shrink(0.0)
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(ExtraRadius::Full.role())
  };
  let outlined = |color: PaletteColor| {
    round()
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, color)
  };
  Some(match node {
    Node::Start => {
      outlined(PaletteColor::BorderStrong).child(icon(Icon::Play, IconSize::Xs, PaletteColor::TextSecondary))
    }
    Node::Step(number) => outlined(PaletteColor::BorderStrong).child(number_text(id, *number)),
    Node::NotReady(number) => outlined(PaletteColor::Warning).child(number_text(id, *number)),
    Node::Gate => {
      round()
        .background(PaletteColor::WarningMuted)
        .child(icon(Icon::Hand, IconSize::Xs, PaletteColor::Warning))
    }
    Node::Parallel => {
      outlined(PaletteColor::BorderStrong).child(icon(Icon::Split, IconSize::Xs, PaletteColor::TextSecondary))
    }
    Node::Line => return None,
    Node::End => outlined(PaletteColor::BorderStrong).child(
      Row::new()
        .width(Dimension::Px(DOT))
        .height(Dimension::Px(DOT))
        .rounded(ExtraRadius::Full.role())
        .background(PaletteColor::Success),
    ),
  })
}

fn number_text(id: &str, number: usize) -> Text {
  Text::new(&format!("{number:02}"))
    .id(format!("{id}-number"))
    .variant(FlowText::Number.role())
}
