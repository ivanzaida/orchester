//! Table header and rows (`Task list header` `FjOYw`, `Task list row` `XEQuo`): cells side by side, 12
//! apart, 12 in from both ends, each row on a 1 `border` line. The header is 32 high with its column
//! headings in 10.5/500 `text_muted`, tracked 0.4 (the screen words them in capitals, as `TASK / TAGS`);
//! a row is 44 high and takes the `hover` fill under the pointer. A row that opens something ends in a
//! 14 `text_muted` chevron, and the header keeps its column. A highlighted row, e.g. the task a Create
//! just added, is tinted `info_muted` (`Row · KON-37` in `t3oAOi`). Rows are pointer-only.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::pressable,
};
use crate::theme::{ExtraColor, ExtraTypography, IconSize};

/// `Task list header` and `Task list row` heights, for a screen that sizes a table by its rows.
pub const HEADER_HEIGHT: f32 = 32.0;
pub const ROW_HEIGHT: f32 = 44.0;
/// Secrets row I2uJV0.
pub const SECRET_ROW_HEIGHT: f32 = 48.0;
/// The open chevron's column (`Open` `aM7lc`).
const OPEN_COLUMN: f32 = 14.0;

/// How wide a column is: a fixed width, or the room the others leave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColumnWidth {
  Fixed(f32),
  Fill,
}

impl ColumnWidth {
  /// `cell` sized for this column.
  fn sized(self, cell: Row) -> Row {
    match self {
      Self::Fixed(width) => cell.width(Dimension::Px(width)),
      Self::Fill => cell.flex(1.0).min_width(Dimension::Px(0.0)),
    }
  }
}

fn line(row: Row, height: f32) -> Row {
  row
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
}

/// The header of a table whose columns are `columns`, each with its width and heading (none for a
/// column of controls, e.g. selection); `openable` keeps the rows' chevron column.
pub fn table_header(id: &str, columns: &[(ColumnWidth, Option<&str>)], openable: bool) -> Row {
  let headings = columns.iter().map(|(width, heading)| {
    width.sized(Row::new()).with_children(heading.map(|heading| {
      Text::new(heading)
        .variant(ExtraTypography::ColumnHeading.role())
        .nowrap()
    }))
  });
  line(Row::new().id(id.to_owned()), HEADER_HEIGHT)
    .with_children(headings)
    .with_children(openable.then(|| Row::new().width(Dimension::Px(OPEN_COLUMN))))
}

pub struct TableRow {
  id: Arc<str>,
  cells: Vec<(ColumnWidth, Element)>,
  open: Option<Arc<dyn Fn() + Send + Sync>>,
  /// The chevron's column stays, empty, in a table whose other rows open.
  open_column: bool,
  highlighted: bool,
  selected: bool,
  secret: bool,
  height: f32,
  spacing: SpacingSize,
}

impl TableRow {
  pub fn new(id: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      cells: Vec::new(),
      open: None,
      open_column: false,
      highlighted: false,
      selected: false,
      secret: false,
      height: ROW_HEIGHT,
      spacing: SpacingSize::Md,
    }
  }

  /// Secret list row (I2uJV0): 48 high, 12 inside and between its columns.
  pub fn secret(mut self) -> Self {
    self.secret = true;
    self
  }

  /// The inspected row is inert and takes the accent-muted fill (Secrets, aio2p).
  pub fn selected(mut self) -> Self {
    self.selected = true;
    self
  }

  /// The pipeline library's taller two-line rows (DSUqE).
  pub fn height(mut self, height: f32) -> Self {
    self.height = height;
    self
  }
  /// Gap between this table's columns.
  pub fn spacing(mut self, spacing: SpacingSize) -> Self {
    self.spacing = spacing;
    self
  }

  /// The next cell, in its column's width.
  pub fn cell(mut self, width: ColumnWidth, content: impl Into<Element>) -> Self {
    self.cells.push((width, content.into()));
    self
  }

  /// A press on the row runs `open`; the row ends in the chevron.
  pub fn on_open(mut self, open: impl Fn() + Send + Sync + 'static) -> Self {
    self.open = Some(Arc::new(open));
    self.open_column = true;
    self
  }

  /// A row that does not open, in a table whose other rows do: its chevron's column stays empty, so its
  /// cells line up with theirs.
  pub fn in_openable_table(mut self) -> Self {
    self.open_column = true;
    self
  }

  /// The row stands out in the `info_muted` tint, e.g. as the one a Create just added.
  pub fn highlighted(mut self) -> Self {
    self.highlighted = true;
    self
  }
}

impl From<TableRow> for Element {
  fn from(row: TableRow) -> Self {
    let cells = row
      .cells
      .into_iter()
      .map(|(width, content)| width.sized(Row::new()).align_items(Alignment::Center).child(content));
    let content = line(Row::new(), row.height).spacing(row.spacing).with_children(cells);
    let content = if row.secret {
      content
        .height(Dimension::Px(SECRET_ROW_HEIGHT))
        .padding(Padding::horizontal(crate::theme::SecretsSpacing::Cell.role()))
        .spacing(crate::theme::SecretsSpacing::Cell.role())
    } else {
      content
    };
    let content = if row.selected {
      content
        .background(PaletteColor::AccentMuted)
        .describe("selected", "true")
    } else {
      content
    };
    let content = if row.highlighted {
      content
        .background(PaletteColor::InfoMuted)
        .class("highlighted")
        .describe("highlighted", "true")
    } else {
      content
    };
    let open = if row.selected { None } else { row.open };
    let Some(open) = open else {
      return content
        .id(row.id.to_string())
        .with_children(row.open_column.then(|| {
          Row::new().width(Dimension::Px(OPEN_COLUMN)).with_children(
            row
              .selected
              .then(|| icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted)),
          )
        }))
        .into();
    };
    pressable(move || open())
      .id(row.id)
      .width(Dimension::Pct(100.0))
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(content.child(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted)))
      .into()
  }
}
