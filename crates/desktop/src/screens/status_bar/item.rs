//! Parts of the status bar items (`pmOpt`): a 22px item with 8px padding and a 6px gap,
//! labels in Inter 11, values in Geist Mono 11, 12px icons and hairline separators. Items are
//! buttons that never take focus: the status bar is pointer-only (see `ui::pressable`).

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Rect, Row, Text, TextOverflow},
  layout::Alignment,
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
/// The slack after a label that gives way ([`yielding`]), and how readily it goes: before the label,
/// by a power of two whose sum with the label's factor stays exact in `f32`.
const SLACK_WIDTH: f32 = 1.0;
const SLACK_GIVES_WAY: f32 = 4_194_304.0;

/// How readily a part of the left group gives way when the bar is narrow, as its flex shrink factor:
/// what the waiting approvals ask first, then which run auto-approves (while no approvals wait), and
/// last the activity and how many approvals wait (factor 1). Beside waiting approvals, which parts
/// show at all is decided by the window's width (`theme::ApprovalsWidths`). lurq shares an overflow by these factors,
/// so a part behind in line gives way only by a fraction of a pixel, which its slack takes
/// ([`truncated_label`]), until the one before it has gone. Powers of two whose sum stays exact in
/// `f32`: lurq subtracts a part's factor once the part reaches its minimum, and a sum that lost the
/// smaller factors would leave the rest of the overflow unshared.
pub const GIVES_WAY_FIRST: f32 = 4_194_304.0;
pub const GIVES_WAY_NEXT: f32 = 2_048.0;

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

/// The start of an item split in two, so that its end can give way alone: lurq shrinks a child of
/// the bar down to a declared minimum, never to the words it holds, so a single item would lose its
/// words too. The start keeps the item's start padding and corners.
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

/// The middle of an item split in three: what gives way first, without padding or corners.
pub fn item_middle(id: &'static str) -> Button {
  surface()
    .id(id)
    .height(Dimension::Px(ITEM_HEIGHT))
    .align_items(Alignment::Center)
}

/// The end of an item split in two ([`item_start`]): what gives way, with the item's end padding
/// and corners.
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
  yielding(
    Text::new(text)
      .variant(TypographyStyle::Caption)
      .color(color)
      .nowrap()
      .text_overflow(TextOverflow::Elipsis),
    1.0,
  )
}

/// Geist Mono 11: counts in `$text`, times, zeros and unknowns in `$subtle`.
pub fn value(text: &str, color: PaletteColor) -> Element {
  Text::new(text)
    .variant(ExtraTypography::MonoValue.role())
    .color(color)
    .nowrap()
    .into()
}

/// A [`value`] that gives way before the label beside it when the bar is narrow, e.g. the time of the
/// latest event.
pub fn yielding_value(text: &str, color: PaletteColor) -> Element {
  yielding(
    Text::new(text)
      .variant(ExtraTypography::MonoValue.role())
      .color(color)
      .nowrap()
      .text_overflow(TextOverflow::Elipsis),
    GIVES_WAY_NEXT,
  )
}

/// `text`, which shrinks by `factor` with an ellipsis, after a pixel of slack. lurq gives every
/// shrinking part its share of an overflow, so a part behind in line (see [`GIVES_WAY_FIRST`]) is
/// given a fraction of a pixel, and an ellipsis would show for it: the slack takes that fraction.
fn yielding(text: Text, factor: f32) -> Element {
  Row::new()
    .flex_shrink(factor)
    .align_items(Alignment::Center)
    .child(text.flex_shrink(1.0))
    .child(Rect::new(Dimension::Px(SLACK_WIDTH), Dimension::Px(SLACK_WIDTH)).flex_shrink(SLACK_GIVES_WAY))
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
