//! Parts of the status bar's items (`Status item` `wnnsD`): 26 high, 8 at each side, a 12 icon and an
//! 11 label in `text_muted`, 6 apart. Items that do something take the `hover` fill under the pointer and
//! never take the focus: the status bar is pointer-only (`ui::pressable`). An action beside an item
//! (*Restart runner*, *Review*, *Stop*) is 11/500 `text_primary`, 3 at its sides, so two actions read 6
//! apart. Text an item shortens keeps its full value in a tooltip (handover §3.3).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, Text},
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Truncated,
  },
};

/// `Status item` height.
pub const ITEM_HEIGHT: f32 = 26.0;
/// The runner's dot (`Dot` `v3Nfvv`).
const DOT_SIZE: f32 = 6.0;

/// The order in which the parts of the bar give way when the window is narrow (lurq `shrink_order`):
/// the bar is one line, so both of its groups give way in one order, each part down to its limit before
/// the next shrinks at all (handover §3.3). The room between the groups goes first, then the words of
/// the window-control item; then what the waiting approval asks, which run auto-approves, the times and
/// clocks, the version, *Tokens today*, the project's name, the counts' labels (together),
/// the activity's words, the words after how many requests wait, and last the runner's words. The hand,
/// the counts, *Review*, *Stop* and MCP never give way.
pub mod give_way {
  pub const ROOM: i32 = -1;
  pub const WINDOW_CONTROL: i32 = 0;
  pub const ASKS: i32 = 1;
  pub const RUN: i32 = 2;
  pub const TIMES: i32 = 3;
  pub const VERSION: i32 = 4;
  pub const TOKENS: i32 = 5;
  pub const PROJECT: i32 = 6;
  pub const COUNT_LABELS: i32 = 7;
  pub const ACTIVITY: i32 = 8;
  pub const APPROVAL_WORDS: i32 = 9;
  pub const RUNNER_WORDS: i32 = 10;
}

/// An item with the element id `id` that shows what it says and does nothing.
pub fn item(id: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
}

/// An item with the element id `id` that runs `press`: the pointer and the `hover` fill.
pub fn pressed(id: &str, press: impl Fn() + Send + Sync + 'static) -> Button {
  pressable(press)
    .id(id.to_owned())
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
}

/// The item's 12 icon.
pub fn glyph(glyph: Icon, color: PaletteColor) -> Element {
  icon(glyph, IconSize::Sm, color).into()
}

/// The item's 11 label, which keeps its words.
pub fn label(text: &str, color: PaletteColor) -> Text {
  Text::new(text).variant(TypographyStyle::Caption).color(color).nowrap()
}

/// A label with the element id `id` that trims its end with an ellipsis when its item gives way, its
/// full value in a tooltip while it is cut.
pub fn cut(ctx: &mut Ctx, id: &str, text: &str, color: PaletteColor) -> Element {
  Truncated::new(id.to_owned(), text.to_owned(), TypographyStyle::Caption)
    .color(color)
    .render(ctx)
}

/// A label that keeps its words or drops out whole when its item gives way.
pub fn dropping(text: &str, color: PaletteColor) -> Text {
  label(text, color).flex_shrink(1.0).shrink_limit(ShrinkLimit::Drop)
}

/// A time or a clock beside an item (*· 14:32*), a part of the bar of its own: it drops out whole with
/// the bar's times.
pub fn time_part(id: &str, time: impl Into<Element>) -> Element {
  Row::new()
    .id(id.to_owned())
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding(Padding::new().right(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .flex_shrink(1.0)
    .shrink_order(give_way::TIMES)
    .shrink_limit(ShrinkLimit::Drop)
    .child(time)
    .into()
}

/// An action beside an item, with the element id `id`.
pub fn action(id: &str, text: &str, press: impl Fn() + Send + Sync + 'static) -> Element {
  pressed(id, press)
    .padding(Padding::horizontal(ExtraSpacing::Fine.role()))
    .describe("a11y_name", text.to_owned())
    .child(
      Text::new(text)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .into()
}

/// The runner's dot in `color`.
pub fn dot(color: PaletteColor) -> Element {
  Row::new()
    .width(Dimension::Px(DOT_SIZE))
    .height(Dimension::Px(DOT_SIZE))
    .rounded(ExtraRadius::Full.role())
    .background(color)
    .into()
}
