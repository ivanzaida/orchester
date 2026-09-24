//! Key/value property (`tfGRt`): a muted label and its value on one line.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Row, Text, TextOverflow},
  layout::layout_kind::Justify,
  node::{Element, dimension::Dimension},
};

pub fn property(label: &str, value: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
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
