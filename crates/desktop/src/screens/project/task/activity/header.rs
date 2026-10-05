//! An entry's one-line header (`N4SlQq`): its title and, at the right, its meta (*exit 0 · 2.1 s*) and how a
//! request about it was answered (*· denied by automation · the agent continued*). When the line is too narrow
//! for all of them (960 with Details open, a long mark), they give way in order (lurq `shrink_order`, as the
//! status bar does): first the room between them, then the title, down to [`TITLE_MIN`], so the command
//! stays readable; then the mark, and last the meta. A problem row's title says what happened, so there
//! the mark and the meta give way first and the title last. The mark and the meta show whole or drop out
//! whole, never cut in a word; the title never disappears.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Row, Spacer, Text, TextOverflow},
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, dimension::Dimension},
};

/// The least a title keeps beside a long meta: a command and its first argument (`git push origin`).
/// No frame draws a narrower title; at 960 with Details open the row's line is about 300 wide.
const TITLE_MIN: f32 = 120.0;
/// The room drops out once it would be narrower than a pixel, so a tight line keeps one gap.
const ROOM_MIN: f32 = 1.0;

/// The order in which the header's parts give way: the lowest first.
mod give_way {
  /// The room that keeps the meta at the right: it drops out, with its spacing, once it is used up.
  pub const ROOM: i32 = -1;
  pub const TITLE: i32 = 0;
  pub const MARK: i32 = 1;
  pub const META: i32 = 2;
  /// A problem row's title.
  pub const TITLE_LAST: i32 = 3;
}

/// The header of entry `sequence`, `height` high, with its chevron or ↗ at the end; `title_last`: its title
/// gives way after the meta and the mark (a problem row).
pub(super) fn header(
  (title, meta, mark): (&str, &str, Option<&str>),
  disclosure: Option<Element>,
  sequence: u64,
  (height, title_last): (f32, bool),
) -> Element {
  let title_order = if title_last {
    give_way::TITLE_LAST
  } else {
    give_way::TITLE
  };
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
        .shrink_order(title_order)
        .selectable(true),
    )
    .child(
      Spacer::new()
        .width(Dimension::Pct(100.0))
        .flex_shrink(1.0)
        .shrink_order(give_way::ROOM)
        .shrink_drop_below(ROOM_MIN),
    )
    .child(whole(meta, format!("activity-entry-meta-{sequence}"), give_way::META))
    .with_children(mark.map(|mark| whole(mark, format!("activity-entry-mark-{sequence}"), give_way::MARK)))
    .with_children(disclosure)
    .into()
}

/// Mono 10 `$subtle` text that keeps its natural width or drops out whole in its `order`.
fn whole(text: &str, id: String, order: i32) -> Element {
  Text::new(text)
    .id(id)
    .variant(TypographyStyle::Mono)
    .color(PaletteColor::TextMuted)
    .nowrap()
    .flex_shrink(1.0)
    .shrink_order(order)
    .shrink_limit(ShrinkLimit::Drop)
    .into()
}
