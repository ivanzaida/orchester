//! The Usage page's tables: each chart's data with exact numbers (`Table` `fxX9i` in `d3lc3`) and *Most
//! expensive* (`Expensive header/task` `C931v9`, `Expensive row/task` `hHWSp`). The header is 28 high with
//! its headings in 11/500 `text_muted`, tracked 0.4 and worded in capitals; a data row is 30 high, a row
//! of *Most expensive* 44; cells side by side from the table's edge, each row on a 1 `border` line. Words
//! in 12 sans, figures in 12 mono at the right of fixed columns; the first column takes the room left.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};

use super::card::swatch;
use crate::{
  theme::{ExtraColor, ExtraSpacing, UsageText},
  ui::pressable::pressable,
};

const HEADER_HEIGHT: f32 = 28.0;
const DATA_ROW_HEIGHT: f32 = 30.0;
const LIST_ROW_HEIGHT: f32 = 44.0;

/// How a column takes its width: a share of the room the fixed columns leave, or a fixed width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Width {
  Flex(f32),
  Px(f32),
}

/// A column: its heading, its width, and whether it holds figures (mono, at its right).
#[derive(Clone, Debug)]
pub struct TableColumn {
  pub heading: Arc<str>,
  pub width: Width,
  pub figures: bool,
  /// The color of the series its figures are, as a swatch before its heading (`CODEX` `UmSkR`).
  pub swatch: Option<PaletteColor>,
}

pub fn column(heading: Arc<str>, width: Width, figures: bool) -> TableColumn {
  TableColumn {
    heading,
    width,
    figures,
    swatch: None,
  }
}

impl TableColumn {
  pub fn swatch(mut self, color: PaletteColor) -> Self {
    self.swatch = Some(color);
    self
  }
}

/// A cell: text in the column's style, or an element of its own.
pub enum Cell {
  Text(Arc<str>),
  /// Text in `text_secondary`: when, a reset, who reported it (`VDZVZ`, `GtXWE`).
  Muted(Arc<str>),
  /// The *Total* row's label and figures (`Total` in `fxX9i`).
  Strong(Arc<str>),
  Element(Element),
}

/// Rows of a table: dense data rows, or *Most expensive*'s list rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
  Data,
  List,
}

impl Density {
  const fn height(self) -> f32 {
    match self {
      Self::Data => DATA_ROW_HEIGHT,
      Self::List => LIST_ROW_HEIGHT,
    }
  }

  /// Between columns: 12 in a chart's data (`xVYTY`), 16 in *Most expensive* (`C931v9`).
  const fn gap(self) -> SpacingSize {
    match self {
      Self::Data => SpacingSize::Md,
      Self::List => SpacingSize::Lg,
    }
  }
}

/// The table `id`: its headings over `rows`.
pub fn table(id: &str, columns: &[TableColumn], density: Density, rows: Vec<Element>) -> Element {
  let headings = Row::new()
    .id(format!("{id}-header"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADER_HEIGHT))
    .spacing(density.gap())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(columns.iter().map(|column| {
      sized(
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .with_children(column.swatch.clone().map(swatch))
          .child(fit(Text::new(&column.heading).variant(UsageText::ColumnHeading.role()))),
        column,
      )
    }));
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .child(headings)
    .with_children(rows)
    .into()
}

/// Row `id` of `cells` under `columns`; its cells are `{id}-cell-{column}`.
pub fn row(id: &str, columns: &[TableColumn], density: Density, cells: Vec<Cell>) -> Row {
  bare_row(id, columns, density, cells).border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
}

/// The *Total* row that ends a table, with no line under it (`Total` `YHSve`).
pub fn total_row(id: &str, columns: &[TableColumn], cells: Vec<Cell>) -> Row {
  bare_row(id, columns, Density::Data, cells)
}

fn bare_row(id: &str, columns: &[TableColumn], density: Density, cells: Vec<Cell>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(density.height()))
    .spacing(density.gap())
    .align_items(Alignment::Center)
    .with_children(columns.iter().zip(cells).enumerate().map(|(index, (column, cell))| {
      sized(
        Row::new().id(format!("{id}-cell-{index}")).child(content(column, cell)),
        column,
      )
    }))
}

/// `row` pressed to run `open`, with the `hover` fill under the pointer.
pub fn opening(row: Row, open: impl Fn() + Send + Sync + 'static) -> Element {
  pressable(open)
    .width(Dimension::Pct(100.0))
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(row)
    .into()
}

fn content(column: &TableColumn, cell: Cell) -> Element {
  let text = |words: &Arc<str>| Text::new(words);
  match cell {
    Cell::Text(words) if column.figures => fit(text(&words).variant(TypographyStyle::Mono)),
    Cell::Text(words) => fit(
      text(&words)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary),
    ),
    Cell::Muted(words) if column.figures => fit(
      text(&words)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary),
    ),
    Cell::Muted(words) => fit(text(&words).variant(TypographyStyle::Description)),
    Cell::Strong(words) if column.figures => fit(text(&words).variant(UsageText::TotalFigure.role())),
    Cell::Strong(words) => fit(text(&words).variant(TypographyStyle::Label)),
    Cell::Element(element) => element,
  }
}

/// One line that gives way with an ellipsis when its column is narrower than the text.
pub fn fit(text: Text) -> Element {
  text
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .into()
}

fn sized(cell: Row, column: &TableColumn) -> Element {
  let cell = cell
    .min_width(Dimension::Px(0.0))
    .align_items(Alignment::Center)
    .justify(if column.figures { Justify::End } else { Justify::Start });
  match column.width {
    // From nothing, not from its content, so long and short names leave later columns in place.
    Width::Flex(share) => cell.flex_full(share, 1.0, Some(0.0)),
    Width::Px(width) => cell.width(Dimension::Px(width)).flex_shrink(0.0),
  }
  .into()
}
