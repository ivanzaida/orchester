//! Read-only source box in the Instructions repository review (U69ob).
use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::icon::{Icon, icon};
use crate::theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, instructions};

/// File header and scrollable, selectable source. Its parent supplies the available height.
pub fn source_box(id: &str, filename: &str, metadata: &str, source: &str) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .background(ExtraColor::CodeBackground.role())
    .rounded(ExtraRadius::Tile.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .clip()
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inline.role()))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(icon(Icon::FileText, IconSize::Md, PaletteColor::TextMuted))
        .child(Text::new(filename).variant(TypographyStyle::Mono).flex(1.0))
        .child(Text::new(metadata).variant(TypographyStyle::Caption)),
    )
    .child(
      ScrollVertical::new(
        Text::new(source)
          .id(format!("{id}-text"))
          .variant(instructions::prompt_text())
          .color(PaletteColor::TextPrimary)
          .selectable(true)
          .width(Dimension::Pct(100.0))
          .padding(ExtraSpacing::Inset.role()),
      )
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0)),
    )
    .into()
}
