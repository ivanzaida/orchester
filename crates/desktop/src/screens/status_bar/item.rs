//! Parts of the status bar items (`pmOpt`): a 22px item with 8px padding and a 6px gap,
//! labels in Inter 11, values in Geist Mono 11, 12px icons and hairline separators. Items are
//! buttons that never take focus: the status bar is pointer-only (see `ui::pressable`).

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Rect, Text, TextOverflow},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension},
};

use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// Item height and separator size (`N8rki`, `i36LJ`).
const ITEM_HEIGHT: f32 = 22.0;
const SEPARATOR_WIDTH: f32 = 1.0;
const SEPARATOR_HEIGHT: f32 = 12.0;

/// An empty item that opens nothing; `id` is for tooling (`lurq_read_tree`, `lurq_interact`).
pub fn item(id: &'static str) -> Button {
  surface()
    .id(id)
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    // The design's 5px item radius; the nearest role is the 6px item radius (see the audit).
    .rounded(RadiusSize::Sm)
}

/// An item that opens something: pointer and the `$hover` fill.
pub fn clickable(item: Button, open: impl Fn() + Send + Sync + 'static) -> Button {
  pressable(item, open).hovered_style(Style::new().background(PaletteColor::AccentMuted))
}

pub fn glyph(glyph: Icon, color: PaletteColor) -> Element {
  icon(glyph, IconSize::Small, color).into()
}

/// Inter 11 label; `$muted` unless emphasized.
pub fn label(text: &str, color: PaletteColor) -> Element {
  Text::new(text)
    .variant(TypographyStyle::Caption)
    .color(color)
    .nowrap()
    .into()
}

/// A label that gives way first when the bar is narrow; the full text is in its hint.
pub fn truncated_label(text: &str, color: PaletteColor) -> Element {
  Text::new(text)
    .variant(TypographyStyle::Caption)
    .color(color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex_shrink(1.0)
    .into()
}

/// Geist Mono 11: counts in `$text`, times, zeros and unknowns in `$subtle`.
pub fn value(text: &str, color: PaletteColor) -> Element {
  Text::new(text)
    .variant(ExtraTypography::MonoValue.role())
    .color(color)
    .nowrap()
    .into()
}

/// Medium 11 action label, e.g. "Restart runner" (`LPDZP`, `l02Wx`).
pub fn action(id: &'static str, text: &str, run: impl Fn() + Send + Sync + 'static) -> Element {
  clickable(
    item(id).child(Text::new(text).variant(TypographyStyle::Link).nowrap()),
    run,
  )
  .into()
}

/// Dismiss control (`npzEn`).
pub fn dismiss(id: &'static str, run: impl Fn() + Send + Sync + 'static) -> Element {
  let button = surface()
    .id(id)
    .size(Dimension::Px(ITEM_HEIGHT), Dimension::Px(ITEM_HEIGHT))
    .align_items(Alignment::Center)
    .justify(lurq::layout::layout_kind::Justify::Center)
    .rounded(RadiusSize::Sm)
    .child(glyph(Icon::X, PaletteColor::TextSecondary));
  clickable(button, run).into()
}

/// Hairline separator between the runner and the activity, and before the version.
pub fn separator() -> Element {
  Rect::new(Dimension::Px(SEPARATOR_WIDTH), Dimension::Px(SEPARATOR_HEIGHT))
    .background(ExtraColor::Hairline.role())
    .into()
}
