//! The dashboard's tables: each chart's data with exact numbers (`CDcij`), and *Most expensive*
//! (`sOQmy`). Sentence-case column headings, rows divided by hairlines, figures in mono at the right.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::theme::{ExtraColor, ExtraTypography};

/// Row heights: *Most expensive* (`sOQmy`) and a chart's data (`CDcij`).
const ROW_HEIGHT: f32 = 40.0;
const DATA_ROW_HEIGHT: f32 = 30.0;
const HEADING_HEIGHT: f32 = 28.0;

/// How a column takes its width: a share of what is left, or a fixed width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Width {
  Flex(f32),
  Px(f32),
}

/// A column: its heading, width, and whether it holds figures (right-aligned, mono).
#[derive(Clone, Debug)]
pub struct TableColumn {
  pub heading: Arc<str>,
  pub width: Width,
  pub figures: bool,
}

pub fn column(heading: Arc<str>, width: Width, figures: bool) -> TableColumn {
  TableColumn {
    heading,
    width,
    figures,
  }
}

/// A cell: text in the column's style, or an element of its own.
pub enum Cell {
  Text(Arc<str>),
  /// Text in `muted`, e.g. a task's title beside its key.
  Muted(Arc<str>),
  /// A total: the label and figures stand out from the rows above.
  Strong(Arc<str>),
  Element(Element),
}

/// The table: headings over `rows`, each row optionally a press target.
pub fn table(id: &'static str, columns: &[TableColumn], rows: Vec<Row>) -> Element {
  let headings = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADING_HEIGHT))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(columns.iter().map(|column| {
      sized(
        Row::new().child(
          Text::new(&column.heading)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextMuted)
            .nowrap(),
        ),
        column,
      )
    }));
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .child(headings)
    .with_children(rows)
    .into()
}

/// One row of `cells` under `columns`, as *Most expensive* lists them.
pub fn row(columns: &[TableColumn], cells: Vec<Cell>) -> Row {
  sized_row(columns, cells, ROW_HEIGHT)
}

/// One row of a chart's data: denser than a list of tasks.
pub fn data_row(columns: &[TableColumn], cells: Vec<Cell>) -> Row {
  sized_row(columns, cells, DATA_ROW_HEIGHT)
}

fn sized_row(columns: &[TableColumn], cells: Vec<Cell>, height: f32) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(
      columns
        .iter()
        .zip(cells)
        .map(|(column, cell)| sized(Row::new().child(content(column, cell)), column)),
    )
}

fn content(column: &TableColumn, cell: Cell) -> Element {
  match cell {
    Cell::Text(text) if column.figures => Text::new(&text)
      .variant(ExtraTypography::MonoValue.role())
      .nowrap()
      .into(),
    Cell::Text(text) => Text::new(&text).variant(TypographyStyle::Body).nowrap().into(),
    Cell::Muted(text) => Text::new(&text)
      .variant(TypographyStyle::Body)
      .color(PaletteColor::TextSecondary)
      .nowrap()
      .into(),
    Cell::Strong(text) if column.figures => Text::new(&text)
      .variant(ExtraTypography::MonoTitle.role())
      .nowrap()
      .into(),
    Cell::Strong(text) => Text::new(&text).variant(TypographyStyle::Label).nowrap().into(),
    Cell::Element(element) => element,
  }
}

fn sized(cell: Row, column: &TableColumn) -> Element {
  let cell = cell
    .min_width(Dimension::Px(0.0))
    .align_items(Alignment::Center)
    .justify(if column.figures { Justify::End } else { Justify::Start });
  match column.width {
    Width::Flex(share) => cell.flex(share),
    Width::Px(width) => cell.width(Dimension::Px(width)).flex_shrink(0.0),
  }
  .into()
}
