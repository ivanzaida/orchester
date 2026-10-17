//! Tool library master `oxod1`: 56-high row, a glyph and two-line name, five fixed cells and an open chevron.
use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::opening,
  table::{ColumnWidth, table_header},
};
use crate::{
  navigation::link::Open,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
};

pub const ROW_HEIGHT: f32 = 56.0;
const GLYPH: f32 = 28.0;
const OPEN: f32 = 14.0;
/// Kind, secrets, version, usage, status widths from `oxod1` and `W325oA`.
pub const WIDTHS: [f32; 5] = [110.0, 130.0, 180.0, 130.0, 100.0];

pub fn header(id: &str, words: &[Arc<str>; 6]) -> Row {
  let columns: Vec<_> = std::iter::once((ColumnWidth::Fill, Some(&*words[0])))
    .chain(
      WIDTHS
        .iter()
        .zip(words.iter().skip(1))
        .map(|(w, t)| (ColumnWidth::Fixed(*w), Some(&**t))),
    )
    .collect();
  table_header(id, &columns, true)
}

pub struct ToolRow {
  pub id: String,
  pub glyph: Icon,
  pub name: Arc<str>,
  pub detail: Arc<str>,
  pub detail_mono: bool,
  pub cells: Vec<Element>,
  /// What opens the tool's page where a press asks (`navigation::link`).
  pub on_open: Open,
}
impl From<ToolRow> for Element {
  fn from(row: ToolRow) -> Self {
    let title = Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(
        Row::new()
          .width(Dimension::Px(GLYPH))
          .height(Dimension::Px(GLYPH))
          .align_items(Alignment::Center)
          .justify(Justify::Center)
          .background(PaletteColor::SurfaceRaised)
          .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .rounded(RadiusSize::Md)
          .child(icon(row.glyph, IconSize::Md, PaletteColor::TextSecondary)),
      )
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(Text::new(&row.name).variant(ExtraTypography::Strong.role()).nowrap())
          .child(
            Text::new(&row.detail)
              .variant(if row.detail_mono {
                ExtraTypography::MonoKey.role()
              } else {
                ExtraTypography::Note.role()
              })
              .nowrap(),
          ),
      );
    let cells = row
      .cells
      .into_iter()
      .zip(WIDTHS)
      .map(|(cell, w)| Row::new().width(Dimension::Px(w)).flex_shrink(0.0).child(cell));
    opening(row.on_open)
      .id(row.id)
      .width(Dimension::Pct(100.0))
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .height(Dimension::Px(ROW_HEIGHT))
          .padding(Padding::horizontal(SpacingSize::Md))
          .spacing(SpacingSize::Md)
          .align_items(Alignment::Center)
          .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .child(title)
          .with_children(cells)
          .child(Row::new().width(Dimension::Px(OPEN)).child(icon(
            Icon::ChevronRight,
            IconSize::Md,
            PaletteColor::TextMuted,
          ))),
      )
      .into()
  }
}
