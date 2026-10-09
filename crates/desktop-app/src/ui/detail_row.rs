//! Detail row (`Detail row` `paUqV`): what a value is in 12 `text_muted` in a 108 column, and the value
//! 12 after it in 12/1.45 `text_primary`, wrapping; 5 over and under them. An identifier's value is in
//! Geist Mono (`Branch` `YHJhy`).

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::theme::{ExtraSpacing, ExtraTypography};

/// The key column (`rtxjz`).
const KEY_WIDTH: f32 = 108.0;

/// How a detail's value is set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DetailValue {
  #[default]
  Text,
  /// An identifier: a branch, a hash, a path.
  Mono,
}

/// `key` and its `value`; the value's element id is `id`, the key's `<id>-key`.
pub fn detail_row(id: &str, key: &str, value: &str, kind: DetailValue) -> Row {
  let typography = match kind {
    DetailValue::Text => ExtraTypography::Prose.role(),
    DetailValue::Mono => TypographyStyle::Mono,
  };
  let value = Text::new(value)
    .id(id.to_owned())
    .variant(typography)
    .color(PaletteColor::TextPrimary)
    .flex(1.0);
  detail_row_of(id, key, value)
}

/// `key` beside a value of more than one line or style, e.g. a lead line over the value (`Detail ·
/// Permissions` `Up8y8`); the key's element id is `<id>-key`, and `value` takes the rest of the row.
pub fn detail_row_of(id: &str, key: &str, value: impl Into<Element>) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Pair.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(
      Text::new(key)
        .id(format!("{id}-key"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(KEY_WIDTH)),
    )
    .child(value)
}
