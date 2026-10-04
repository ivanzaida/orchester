//! The tables of the servers screen (`B8KV9t` sessions, `DNzy9` users): a 28 high heading row of
//! uppercase 9px column names, then rows, each 12 in from the sides with 16 between the cells and a
//! hairline under it.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, SpacingSize},
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// The heading row's height (`Q0nQrg`).
const HEADING_HEIGHT: f32 = 28.0;

/// A column: the one that takes the room left, or a fixed width.
#[derive(Clone, Copy)]
pub(super) enum Width {
  Fill,
  Px(f32),
}

/// A cell's content and where it sits.
pub(super) struct Cell {
  pub width: Width,
  pub end: bool,
  pub content: Element,
}

impl Cell {
  pub fn new(width: Width, content: impl Into<Element>) -> Self {
    Self {
      width,
      end: false,
      content: content.into(),
    }
  }

  /// Content at the cell's end, e.g. the row's actions.
  pub fn at_end(mut self) -> Self {
    self.end = true;
    self
  }
}

pub(super) fn heading(columns: impl IntoIterator<Item = (Width, Option<Arc<str>>)>) -> Element {
  row(
    Dimension::Px(HEADING_HEIGHT),
    columns.into_iter().map(|(width, name)| {
      let content: Element = match name {
        Some(name) => Text::new(&name.to_uppercase())
          .variant(ExtraTypography::ColumnHeading.role())
          .nowrap()
          .into(),
        None => Element::new(),
      };
      Cell::new(width, content)
    }),
  )
}

/// A row `height` high.
pub(super) fn row(height: Dimension, cells: impl IntoIterator<Item = Cell>) -> Element {
  let cells = cells.into_iter().map(|cell| {
    let column = Row::new()
      .height(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .overflow_visible()
      .child(cell.content);
    let column = if cell.end { column.justify(Justify::End) } else { column };
    match cell.width {
      Width::Fill => column.flex(1.0).min_width(Dimension::Px(0.0)),
      Width::Px(width) => column.width(Dimension::Px(width)).flex_shrink(0.0),
    }
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(height)
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(cells)
    .into()
}

/// The rows under their heading.
pub(super) fn table(id: &'static str, heading: Element, rows: impl IntoIterator<Item = Element>) -> Element {
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .child(heading)
    .with_children(rows)
    .into()
}
