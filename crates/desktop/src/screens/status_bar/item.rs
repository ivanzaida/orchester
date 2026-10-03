//! Parts of the status bar items (`pmOpt`): a 22px item with 8px padding and a 6px gap,
//! labels in Inter 11, values in Geist Mono 11, 12px icons and hairline separators. Items are
//! buttons that never take focus: the status bar is pointer-only (see `ui::pressable`).

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Rect, Text, TextOverflow},
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, Style, dimension::Dimension},
};

use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    link::Press,
    pressable::surface,
  },
};

/// Item height and separator size (`N8rki`, `i36LJ`).
const ITEM_HEIGHT: f32 = 22.0;
const SEPARATOR_WIDTH: f32 = 1.0;
const SEPARATOR_HEIGHT: f32 = 12.0;

/// The order in which the parts of the left group give way when the bar is narrow (lurq
/// `shrink_order`): each order gives way, down to its limit, before the next shrinks at all. What the
/// waiting approvals ask trims its end first; then which run auto-approves and the times drop out
/// whole; then the activity drops its time and trims its words; then the words beside how many
/// approvals wait drop out, and last the runner's words. The hand, the count, *Review*,
/// *Auto-approving* and Stop never give way.
pub mod give_way {
  pub const ASKS: i32 = 0;
  pub const RUN: i32 = 1;
  pub const TIMES: i32 = 2;
  pub const ACTIVITY: i32 = 3;
  pub const COUNT_WORDS: i32 = 4;
  pub const RUNNER_WORDS: i32 = 5;
}

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

/// The start of an item split in parts that give way in different orders. The start keeps the item's
/// start padding and corners.
pub fn item_start(id: &'static str) -> Button {
  surface()
    .id(id)
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding_left(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .corner_radius_top_left(RadiusSize::Sm)
    .corner_radius_bottom_left(RadiusSize::Sm)
}

/// The middle of an item split in parts ([`item_start`]), without padding or corners.
pub fn item_middle(id: &'static str) -> Button {
  surface()
    .id(id)
    .height(Dimension::Px(ITEM_HEIGHT))
    .align_items(Alignment::Center)
}

/// The end of an item split in parts ([`item_start`]), with the item's end padding and corners.
pub fn item_end(id: &'static str) -> Button {
  surface()
    .id(id)
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding_right(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .corner_radius_top_right(RadiusSize::Sm)
    .corner_radius_bottom_right(RadiusSize::Sm)
}

/// An item that runs `run`: pointer and the `$hover` fill.
pub fn clickable(item: Button, run: impl Fn() + Send + Sync + 'static) -> Button {
  pressed(item, Press::action(run))
}

/// An item that opens a place where the press asks, or runs an action (see `ui::link`): pointer
/// and the `$hover` fill.
pub fn pressed(item: Button, press: Press) -> Button {
  press
    .on(item)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
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

/// A [`value`] that keeps its natural size or drops out whole when its line gives way (lurq
/// `ShrinkLimit::Drop`), e.g. a time: it is never cut to a stub.
pub fn dropping_value(text: &str, color: PaletteColor) -> Element {
  Text::new(text)
    .variant(ExtraTypography::MonoValue.role())
    .color(color)
    .nowrap()
    .flex_shrink(1.0)
    .shrink_limit(ShrinkLimit::Drop)
    .into()
}

/// A [`label`] that keeps its natural size or drops out whole when its line gives way.
pub fn dropping_label(text: &str, color: PaletteColor) -> Element {
  Text::new(text)
    .variant(TypographyStyle::Caption)
    .color(color)
    .nowrap()
    .flex_shrink(1.0)
    .shrink_limit(ShrinkLimit::Drop)
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
pub fn action(id: &'static str, text: &str, press: Press) -> Element {
  pressed(
    item(id).child(Text::new(text).variant(TypographyStyle::Link).nowrap()),
    press,
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
