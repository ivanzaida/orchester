//! The gallery's sections. Each shows controls in cells captioned with the design component they draw
//! and its Pencil node, so a verifier finds the node to compare with.

mod actions;
mod blocks;
mod decisions;
mod inputs;
mod labels;
mod overlays;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::gallery::{Section, State};

pub fn section(ctx: &mut Ctx, section: Section, state: &State) -> Element {
  match section {
    Section::Actions => actions::section(ctx, state),
    Section::Inputs => inputs::section(ctx, state),
    Section::Labels => labels::section(ctx, state),
    Section::Blocks => Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xl)
      .child(blocks::section(ctx, state))
      .child(overlays::section(ctx, state))
      .into(),
    Section::Decisions => decisions::section(ctx, state),
  }
}

/// `control` under its caption.
fn cell(caption: &str, control: impl Into<Element>) -> Column {
  Column::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(Text::new(caption).variant(TypographyStyle::Caption))
    .child(control)
}

/// `cell` filling `width`.
fn wide_cell(caption: &str, width: f32, control: impl Into<Element>) -> Column {
  cell(caption, control)
    .width(Dimension::Px(width))
    .align_items(Alignment::Stretch)
}

/// Cells side by side, aligned at their tops.
fn line(cells: impl IntoIterator<Item = Column>) -> Row {
  Row::new()
    .spacing(SpacingSize::Section)
    .align_items(Alignment::Start)
    .with_children(cells)
}

/// The section's parts one under the other.
fn stack(parts: impl IntoIterator<Item = Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xl)
    .with_children(parts)
    .into()
}
