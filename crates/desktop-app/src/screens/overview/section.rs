//! What the Overview's sections share (`Head · At work` `SwaLB`, `KON-16` `IJRfV`, `KON-6` `F3wZk` in 01 `LXhun`): a
//! head 28 high and 8 in at its sides, its name in 13/600, its count in 12 `text_muted` and at its end a link
//! (12/500 `text_secondary`, *Accounts ↗*) or a control; its lines 2 apart under it, each 40 high (30 in the side
//! column), 8 in, radius 6 with the `hover` fill; the task's key in Geist Mono 11.5 on 52; how long, in Geist Mono 11
//! (*12m*); and *Show all N* where a list is cut.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::time::Timestamp;

use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, OverviewText},
  ui::pressable::{Press, pressed},
};

/// A section head's height (`SwaLB`), a line's in the main column (`IJRfV`) and in the side column (`F3wZk`).
const HEAD_HEIGHT: f32 = 28.0;
pub(super) const LINE_HEIGHT: f32 = 40.0;
pub(super) const SIDE_LINE_HEIGHT: f32 = 30.0;
/// The key's room (`TdXZC`).
const KEY_WIDTH: f32 = 52.0;

/// A section named `id`: its head, then its lines 2 apart.
pub(super) fn section(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
}

/// A section's head: `title`, its `count` when it has one, and `end` at its end.
pub(super) fn head(id: &str, title: &str, count: Option<String>, end: Option<Element>) -> Row {
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(OverviewText::Section.role())
        .nowrap(),
    )
    .with_children(count.map(|count| muted(&count).id(format!("{id}-count"))))
    .child(Spacer::new().flex(1.0))
    .with_children(end)
}

/// Words in 12 `text_muted`.
pub(super) fn muted(words: &str) -> Text {
  Text::new(words)
    .variant(TypographyStyle::Description)
    .color(PaletteColor::TextMuted)
    .nowrap()
}

/// A link of words (*Accounts ↗*), named `id`.
pub(super) fn link(id: &str, label: &str, press: Press) -> Element {
  pressed(press)
    .id(id.to_owned())
    .describe("a11y_name", label.to_owned())
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .child(
      Text::new(label)
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .into()
}

/// A line of a section, named `id`, 8 in, its parts `spacing` apart.
pub(super) fn line(id: &str, height: f32, spacing: SpacingSize) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(spacing)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
}

/// A task's key in its room.
pub(super) fn key(key: &str) -> Row {
  Row::new()
    .width(Dimension::Px(KEY_WIDTH))
    .flex_shrink(0.0)
    .child(Text::new(key).variant(ExtraTypography::MonoKey.role()).nowrap())
}

/// A task's title, cut to its room, in `color`.
pub(super) fn title(id: &str, title: &str, style: TypographyStyle, color: PaletteColor) -> Text {
  Text::new(title)
    .id(id.to_owned())
    .variant(style)
    .color(color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
}

/// A time in Geist Mono 11 `text_muted`.
pub(super) fn mono_time(id: &str, time: &str) -> Text {
  Text::new(time)
    .id(id.to_owned())
    .variant(ExtraTypography::MonoNote.role())
    .nowrap()
    .flex_shrink(0.0)
}

/// How long since `since` at `now`, in its largest whole unit: *23m*, *6h*, *1d*.
pub(super) fn age(ctx: &Ctx, since: Timestamp, now: Timestamp) -> Arc<str> {
  const MINUTE: i64 = 60_000;
  let millis = now.unix_millis() - since.unix_millis();
  let (key, count) = match millis {
    ..MINUTE => return ctx.t("overview.age.now"),
    MINUTE..3_600_000 => ("overview.age.minutes", millis / MINUTE),
    3_600_000..86_400_000 => ("overview.age.hours", millis / 3_600_000),
    _ => ("overview.age.days", millis / 86_400_000),
  };
  ctx.t_args(key, [("count", count.to_string())])
}

/// *Show all N in Tasks ↗*, a line under a cut list.
pub(super) fn show_all(id: &str, label: &str, press: Press) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Sm))
    .child(link(id, label, press))
}
