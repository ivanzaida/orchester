//! An entry's one-line header (`N4SlQq`): its title and, at the right, its meta with any decision mark
//! (*exit 0 · 2.1 s · denied by automation · the agent continued*). When the line is too narrow for both
//! (960 with Details open, a long mark), they give way in order (lurq `shrink_order`, as the status bar
//! does): first the room between them, then the title, down to [`TITLE_MIN`], so the command stays
//! readable; then the meta, which ends in an ellipsis and drops out whole once it would be narrower than
//! [`META_MIN`]. The title never disappears, and the meta is never cut without its ellipsis.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

/// The least a title keeps beside a long meta: a command and its first argument (`git push origin`).
/// No frame draws a narrower title; at 960 with Details open the row's line is about 300 wide.
const TITLE_MIN: f32 = 120.0;
/// Narrower than this, the meta drops out rather than show a stub before its ellipsis.
const META_MIN: f32 = 48.0;
/// The room drops out once it would be narrower than a pixel, so a tight line keeps one gap.
const ROOM_MIN: f32 = 1.0;

/// The order in which the header's parts give way: the lowest first.
mod give_way {
  /// The room that keeps the meta at the right: it drops out, with its spacing, once it is used up.
  pub const ROOM: i32 = -1;
  pub const TITLE: i32 = 0;
  pub const META: i32 = 1;
}

/// The header of entry `sequence`, `height` high, with its chevron or ↗ at the end.
pub(super) fn header((title, meta): (&str, &str), disclosure: Option<Element>, sequence: u64, height: f32) -> Element {
  Row::new()
    .id(format!("activity-entry-header-{sequence}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(title)
        .id(format!("activity-entry-title-{sequence}"))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(TITLE_MIN))
        .shrink_order(give_way::TITLE)
        .selectable(true),
    )
    .child(
      Spacer::new()
        .width(Dimension::Pct(100.0))
        .flex_shrink(1.0)
        .shrink_order(give_way::ROOM)
        .shrink_drop_below(ROOM_MIN),
    )
    .child(
      Text::new(meta)
        .id(format!("activity-entry-meta-{sequence}"))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .shrink_order(give_way::META)
        .shrink_drop_below(META_MIN),
    )
    .with_children(disclosure)
    .into()
}
