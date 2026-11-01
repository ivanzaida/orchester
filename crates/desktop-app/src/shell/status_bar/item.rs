//! Parts of the status bar's items (`Status item` `wnnsD`): 26 high, 8 at each side, a 12 icon and an
//! 11 label in `text_muted`, 6 apart. Items that do something take the `hover` fill under the pointer and
//! never take the focus: the status bar is pointer-only (`ui::pressable`). An action (*Restart runner*,
//! *Review*, *Stop*) is the compact button (`Button/Size/Compact` `SHKod`), 6 from the next one. Text an
//! item shortens keeps its full value in a tooltip (handover §3.3).

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
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::Press,
    tooltip::Truncated,
  },
};

/// `Status item` height.
pub const ITEM_HEIGHT: f32 = 26.0;
/// The runner's dot (`Dot` `v3Nfvv`).
const DOT_SIZE: f32 = 6.0;

/// The order in which the parts of the bar give way when the window is narrow (lurq `shrink_order`):
/// the bar is one line, so both of its groups give way in one order, each part down to its limit before
/// the next shrinks at all (handover §3.3, `Narrow` in `Status bar · redesign`). The room between the groups
/// goes first, then the words of the window-control item and of the Browser item; then which run auto-approves, the
/// times and clocks, the activity's words, and last the runner's words. Icons, *Open*, *Stop*, MCP and the health dot
/// never give way.
pub mod give_way {
  pub const ROOM: i32 = -1;
  pub const WINDOW_CONTROL: i32 = 0;
  pub const BROWSER: i32 = 1;
  pub const RUN: i32 = 2;
  pub const TIMES: i32 = 3;
  pub const ACTIVITY: i32 = 8;
  pub const RUNNER_WORDS: i32 = 10;
}

/// An item with the element id `id` that shows what it says and does nothing.
pub fn item(id: &str) -> Row {
  frame(Row::new().id(id.to_owned()))
}

/// `row` laid out as an item.
pub fn frame(row: Row) -> Row {
  row
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
}

/// An item with the element id `id` pressed as `press` says, an action or a place it opens: the pointer and the
/// `hover` fill.
pub fn pressed(id: &str, press: Press) -> Button {
  press
    .button()
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

/// An action beside an item, with the element id `id`: the compact button, 6 before what follows it.
pub fn action(id: &str, text: &str, press: Press) -> Element {
  Row::new()
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding(Padding::new().right(ExtraSpacing::Snug.role()))
    .align_items(Alignment::Center)
    .child(button(id, text, press))
    .into()
}

/// The compact button (`Button/Size/Compact` `SHKod`) with the element id `id`, inside an item: 22 high,
/// `surface_raised` inside a `border_strong` stroke, an 11.5 `text_primary` label, 10 at each side.
pub fn button(id: &str, text: &str, press: Press) -> Element {
  ActionButton::new(id.to_owned(), ButtonKind::Secondary, text.to_owned())
    .compact()
    .tight()
    .press(press)
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
