//! Key/value property (`tfGRt`): a muted label and its value on one line.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};

/// Row height (`tfGRt`).
const PROPERTY_HEIGHT: f32 = 20.0;

pub fn property(label: &str, value: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PROPERTY_HEIGHT))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .justify(Justify::SpaceBetween)
    .child(Text::new(label).variant(TypographyStyle::Caption).nowrap())
    .child(
      Text::new(value)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .selectable(true),
    )
    .into()
}
