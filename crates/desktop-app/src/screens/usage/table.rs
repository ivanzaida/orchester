//! The Usage page's tables: each chart's data with exact numbers (`Table` `fxX9i` in `d3lc3`) and *Most
//! expensive* (`Expensive header/task` `C931v9`, `Expensive row/task` `hHWSp`). The header is 28 high with
//! its headings in 11/500 `text_muted`, tracked 0.4 and worded in capitals; a data row is 30 high, a row
//! of *Most expensive* 44; cells side by side from the table's edge, each row on a 1 `border` line. Words
//! in 12 sans, figures in 12 mono at the right of fixed columns; the first column takes the room left.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::usage::AccountSeries;

use super::series::series_swatch;
use crate::{
  navigation::link::Open,
  theme::{ExtraColor, ExtraSpacing, UsageText},
};

const HEADER_HEIGHT: f32 = 28.0;
/// A header whose headings have a line over them (`FdC7Y`).
const TALL_HEADER_HEIGHT: f32 = 44.0;
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
  /// A line over the heading: the CLI over its account's name (`CODEX` over `CLIENT` in `FdC7Y`).
  pub over: Option<Arc<str>>,
  pub width: Width,
  pub figures: bool,
  /// The series its figures are, whose swatch goes before its heading (`CLIENT` in `FdC7Y`).
  pub swatch: Option<AccountSeries>,
}

pub fn column(heading: Arc<str>, width: Width, figures: bool) -> TableColumn {
  TableColumn {
    heading,
    over: None,
    width,
    figures,
    swatch: None,
  }
}

impl TableColumn {
  pub const fn swatch(mut self, series: AccountSeries) -> Self {
    self.swatch = Some(series);
    self
  }

  pub fn over(mut self, over: Option<Arc<str>>) -> Self {
    self.over = over;
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

/// Rows of a table: dense data rows, *Plan limits*' readings, or *Most expensive*'s list rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
  Data,
  Readings,
  List,
}

impl Density {
  const fn height(self) -> f32 {
    match self {
      Self::Data | Self::Readings => DATA_ROW_HEIGHT,
      Self::List => LIST_ROW_HEIGHT,
    }
  }

  /// Between columns: 12 in a chart's data (`xVYTY`), 16 in *Most expensive* (`C931v9`).
  const fn gap(self) -> SpacingSize {
    match self {
      Self::Data => SpacingSize::Md,
      // 8 between *Plan limits*' readings (`CFzvr`).
      Self::Readings => SpacingSize::Sm,
      Self::List => SpacingSize::Lg,
    }
  }
}

/// The table `id`: its headings over `rows`.
pub fn table(ctx: &Ctx, id: &str, columns: &[TableColumn], density: Density, rows: Vec<Element>) -> Element {
  let gap = (density.gap(), ctx.theme().spacing_value(density.gap()).to_px());
  // Headings with a line over them make the header taller; every heading then sits on its bottom.
  let tall = columns.iter().any(|column| column.over.is_some());
  let headings = Row::new()
    .id(format!("{id}-header"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(if tall { TALL_HEADER_HEIGHT } else { HEADER_HEIGHT }))
    .padding(if tall {
      Padding::new().bottom(ExtraSpacing::TextArea.role())
    } else {
      Padding::new()
    })
    .align_items(if tall { Alignment::End } else { Alignment::Center })
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(
      columns
        .iter()
        .enumerate()
        .map(|(index, column)| heading(&format!("{id}-heading-{index}"), column, (index > 0).then_some(gap))),
    );
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .child(headings)
    .with_children(rows)
    .into()
}

/// A column's heading `id`: its swatch and words `{id}-words`, with the line over them where it has one; its
/// cell takes the `gap` before its column too.
fn heading(id: &str, column: &TableColumn, gap: Option<(SpacingSize, f32)>) -> Element {
  let whole = column.swatch.is_some();
  // A series' heading is shown whole, past its column's room if it must (*2 ACCOUNTS*, *NO ACCOUNT* in
  // `FdC7Y`); other headings give way with an ellipsis.
  let text = Text::new(&column.heading).variant(UsageText::ColumnHeading.role());
  let text = if whole {
    text.nowrap().flex_shrink(0.0).into()
  } else {
    fit(text)
  };
  let words = Row::new()
    .id(format!("{id}-words"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .flex_shrink(if whole { 0.0 } else { 1.0 })
    .min_width(Dimension::Px(0.0))
    .with_children(column.swatch.map(series_swatch))
    .child(text);
  let justify = if column.figures { Justify::End } else { Justify::Start };
  let line = |child: Element| Row::new().width(Dimension::Pct(100.0)).justify(justify).child(child);
  let cell = Column::new()
    .spacing(ExtraSpacing::Fine.role())
    .with_children(
      column
        .over
        .as_ref()
        .map(|over| line(Text::new(over).variant(UsageText::ColumnOver.role()).nowrap().into())),
    )
    .child(line(words.into()));
  sized_column(cell.id(id.to_owned()), column, gap)
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
      let cell_id = format!("{id}-cell-{index}");
      sized(
        Row::new()
          .child(content(&format!("{cell_id}-text"), column, cell))
          .id(cell_id),
        column,
      )
    }))
}

/// `row` opening its place with `open` by a browser's rules (`navigation::link`), with the `hover` fill under the
/// pointer.
pub fn opening(row: Row, open: Open) -> Element {
  crate::ui::pressable::opening(open)
    .width(Dimension::Pct(100.0))
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(row)
    .into()
}

/// A cell's content; its text is `id`.
fn content(id: &str, column: &TableColumn, cell: Cell) -> Element {
  let text = |words: &Arc<str>| Text::new(words).id(id.to_owned());
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
    Cell::Strong(words) => fit(text(&words).variant(UsageText::TotalLabel.role())),
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

/// A heading's cell: its column and the `gap` before it. Every box clips what runs past it, so a fixed figure
/// column's heading takes the gap as room (*NO ACCOUNT* over an 84 column in `d3lc3`); other headings keep
/// it as padding and start where their column does.
fn sized_column(cell: Column, column: &TableColumn, gap: Option<(SpacingSize, f32)>) -> Element {
  let cell = cell.min_width(Dimension::Px(0.0));
  let room = matches!(column.width, Width::Px(_)) && column.figures;
  let cell = match gap {
    Some((role, _)) if !room => cell.padding(Padding::new().left(role)),
    _ => cell,
  };
  match column.width {
    Width::Flex(share) => cell.flex_full(share, 1.0, Some(0.0)),
    Width::Px(width) => cell
      .width(Dimension::Px(width + gap.map_or(0.0, |(_, px)| px)))
      .flex_shrink(0.0),
  }
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
