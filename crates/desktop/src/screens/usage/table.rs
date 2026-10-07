//! The dashboard's tables: each chart's data with exact numbers (`CDcij`), and *Most expensive*
//! (`sOQmy`). Sentence-case column headings, rows divided by hairlines, figures in mono at the right.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::theme::{ExtraColor, ExtraTypography};

/// Row heights: *Most expensive* (`sOQmy`) and a chart's data (`CDcij`).
const ROW_HEIGHT: f32 = 40.0;
const DATA_ROW_HEIGHT: f32 = 30.0;
const HEADING_HEIGHT: f32 = 28.0;

/// How a column takes its width: a share of what is left, a fixed width, or as wide as its heading
/// and its widest cell, so neither is cut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Width {
  Flex(f32),
  Px(f32),
  Fit,
}

/// Builds a [`Width::Fit`] column's widest cell again, for every row to take its width from: rows
/// are laid out one by one, so each holds the column's heading and widest cell, invisible and
/// without height.
#[derive(Clone)]
pub struct Sizer(Arc<dyn Fn() -> Element + Send + Sync>);

impl Sizer {
  pub fn new(widest: impl Fn() -> Element + Send + Sync + 'static) -> Self {
    Self(Arc::new(widest))
  }
}

impl std::fmt::Debug for Sizer {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("Sizer")
  }
}

/// A column: its heading, width, whether it holds figures (right-aligned, mono), and for a
/// [`Width::Fit`] column its widest cell.
#[derive(Clone, Debug)]
pub struct TableColumn {
  pub heading: Arc<str>,
  pub width: Width,
  pub figures: bool,
  pub sizer: Option<Sizer>,
}

pub fn column(heading: Arc<str>, width: Width, figures: bool) -> TableColumn {
  TableColumn {
    heading,
    width,
    figures,
    sizer: None,
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

/// Gives each [`Width::Fit`] column that has no sizer yet the longest text of its cells in `rows`,
/// in the style the column draws it with.
pub fn size_to(columns: &mut [TableColumn], rows: &[Vec<Cell>]) {
  for (index, column) in columns.iter_mut().enumerate() {
    if column.width != Width::Fit || column.sizer.is_some() {
      continue;
    }
    let longest = rows
      .iter()
      .filter_map(|cells| match cells.get(index)? {
        Cell::Text(text) | Cell::Muted(text) | Cell::Strong(text) => Some(text.clone()),
        Cell::Element(_) => None,
      })
      .max_by_key(|text| text.chars().count());
    let figures = column.figures;
    column.sizer = longest.map(|text| {
      Sizer::new(move || {
        let variant = if figures {
          ExtraTypography::MonoValue.role()
        } else {
          TypographyStyle::Body
        };
        Text::new(&text).variant(variant).nowrap().into()
      })
    });
  }
}

/// The table: headings over `rows`, each row optionally a press target.
pub fn table(id: &'static str, columns: &[TableColumn], rows: Vec<Row>) -> Element {
  spaced_table(id, columns, rows, SpacingSize::Md)
}

/// [`table`] with `gap` between its columns, as its rows were built with.
pub fn spaced_table(id: &'static str, columns: &[TableColumn], rows: Vec<Row>, gap: SpacingSize) -> Element {
  let headings = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADING_HEIGHT))
    .spacing(gap)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(columns.iter().enumerate().map(|(index, column)| {
      sized(
        Row::new().child(fit(heading(column).id(format!("{id}-heading-{index}")))),
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

/// One row of `cells` under `columns`, `gap` apart, as *Most expensive* lists them.
pub fn row(columns: &[TableColumn], cells: Vec<Cell>, gap: SpacingSize) -> Row {
  sized_row(columns, cells, (ROW_HEIGHT, gap))
}

/// One row of a chart's data: denser than a list of tasks.
pub fn data_row(columns: &[TableColumn], cells: Vec<Cell>) -> Row {
  sized_row(columns, cells, (DATA_ROW_HEIGHT, SpacingSize::Md))
}

fn sized_row(columns: &[TableColumn], cells: Vec<Cell>, (height, gap): (f32, SpacingSize)) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .spacing(gap)
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
    Cell::Text(text) if column.figures => fit(Text::new(&text).variant(ExtraTypography::MonoValue.role())),
    Cell::Text(text) => fit(Text::new(&text).variant(TypographyStyle::Body)),
    Cell::Muted(text) => fit(
      Text::new(&text)
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    ),
    Cell::Strong(text) if column.figures => fit(Text::new(&text).variant(ExtraTypography::MonoTitle.role())),
    Cell::Strong(text) => fit(Text::new(&text).variant(TypographyStyle::Label)),
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

fn heading(column: &TableColumn) -> Text {
  Text::new(&column.heading)
    .variant(TypographyStyle::Caption)
    .color(PaletteColor::TextMuted)
}

fn sized(cell: Row, column: &TableColumn) -> Element {
  let cell = cell
    .min_width(Dimension::Px(0.0))
    .align_items(Alignment::Center)
    .justify(if column.figures { Justify::End } else { Justify::Start });
  match column.width {
    Width::Flex(share) => cell.flex(share).into(),
    Width::Px(width) => cell.width(Dimension::Px(width)).flex_shrink(0.0).into(),
    Width::Fit => fitted(cell, column),
  }
}

/// `cell` over the column's heading and widest cell, which give it their width and are not seen.
fn fitted(cell: Row, column: &TableColumn) -> Element {
  let room = Column::new()
    .height(Dimension::Px(0.0))
    .clip()
    .opacity(0.0)
    .child(heading(column).nowrap())
    .with_children(column.sizer.as_ref().map(|sizer| (sizer.0)()));
  Column::new()
    .flex_shrink(0.0)
    .align_items(if column.figures {
      Alignment::End
    } else {
      Alignment::Start
    })
    .child(cell)
    .child(room)
    .into()
}
