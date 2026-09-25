//! Key/value property (`tfGRt`): a muted label and its value on one line.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};

use crate::theme::ExtraTypography;

/// Row height (`tfGRt`).
const PROPERTY_HEIGHT: f32 = 20.0;

/// How a property value reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueStyle {
  Text,
  /// An identifier in mono: a remote, a branch, a path.
  Mono,
  /// Not known (yet), in the subtle color: *Checking…*, *Not signed in*.
  Pending,
}

pub fn property(label: &str, value: &str) -> Element {
  styled_property(label, value, ValueStyle::Text)
}

pub fn styled_property(label: &str, value: &str, style: ValueStyle) -> Element {
  let (typography, color) = match style {
    ValueStyle::Text => (TypographyStyle::Caption, PaletteColor::TextPrimary),
    ValueStyle::Mono => (ExtraTypography::MonoValue.role(), PaletteColor::TextPrimary),
    ValueStyle::Pending => (TypographyStyle::Caption, PaletteColor::TextMuted),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PROPERTY_HEIGHT))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .justify(Justify::SpaceBetween)
    .child(Text::new(label).variant(TypographyStyle::Caption).nowrap())
    .child(
      Text::new(value)
        .variant(typography)
        .color(color)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .selectable(true),
    )
    .into()
}
