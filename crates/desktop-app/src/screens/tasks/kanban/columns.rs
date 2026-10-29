//! The board without rows (`01 Board` `ntozw`): its columns side by side, their heads pinned at the top and their cards
//! under them in one scroll, whose one scrollbar runs at the board's right edge in the room the page leaves there, as
//! the list's does (the design draws none; the person asked for one for the whole board). The cards of every column
//! move together, and a shorter column simply ends. Three layers, laid out alike so their columns line up: under
//! everything the columns' frames (their heads, lit while a lifted card is over them, the drop target of each over
//! its whole visible height, and the folded columns' strips); over them the cards in the scroll; and over both what a
//! drag or a refusal draws over a column's cards, which stays over the part of them that shows. The upper layers let
//! the pointer through where they draw nothing, so the heads and the strips' Unfold under them take their presses.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, ScrollVertical, Stack},
  layout::{StackAlignment, layout_kind::ScrollState},
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};
use orchester_core::task::organization::WorkflowStatus;

use super::{
  actions::BoardState,
  column::{HEAD_HEIGHT, STRIP_WIDTH, Shared, parts, strip},
  folded,
  model::{ColumnKey, ColumnModel},
};
use crate::theme::{ExtraSpacing, TasksSpacing, list_scrollbar};

/// The columns' heads over their cards, which scroll as one where `scroll` says.
pub fn columns(
  ctx: &mut Ctx,
  (state, shared): (&BoardState, &Shared<'_>),
  columns: &[ColumnModel],
  scroll: &ScrollState,
) -> Element {
  let (mut frames, mut cards, mut overs, mut drawn) = (Vec::new(), Vec::new(), Vec::new(), false);
  for column in columns {
    if folded(shared.display, &column.key) {
      // The collapsed Canceled strip stands out in a stronger stroke (`05 Collapsed and empty`).
      let stroke = if column.key == ColumnKey::Status(WorkflowStatus::Canceled) {
        PaletteColor::BorderStrong
      } else {
        PaletteColor::Border
      };
      frames.push(strip(ctx, state, column, stroke));
      cards.push(strip_room());
      overs.push(strip_room());
      continue;
    }
    let parts = parts(ctx, (state, shared), (column, &column.cards));
    drawn |= parts.over.is_some();
    frames.push(parts.frame);
    cards.push(parts.cards);
    overs.push(over_cards(parts.over));
  }
  let over = drawn.then(|| {
    layer(overs)
      .height(Dimension::Pct(100.0))
      .hit_test(HitTestBehavior::ContentOnly)
  });
  let scrolled = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .hit_test(HitTestBehavior::ContentOnly)
    .child(head_room())
    .child(
      ScrollVertical::new(layer(cards))
        .with_scroll_state(scroll.clone())
        .scrollbar(list_scrollbar(&ctx.theme().scrollbar()))
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0)),
    );
  Stack::new()
    .id("tasks-board-columns")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .stack_align(StackAlignment::TopStart)
    .child(layer(frames).height(Dimension::Pct(100.0)))
    .child(scrolled)
    .with_children(over)
    .into()
}

/// One layer's columns, 28 in from the board's right edge, where the scrollbar runs.
fn layer(columns: impl IntoIterator<Item = Element>) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().right(TasksSpacing::Sides.role()))
    .spacing(ExtraSpacing::Loose.role())
    .with_children(columns)
}

/// Room as high as a column's head, which lets the pointer through to the head under it.
fn head_room() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .flex_shrink(0.0)
    .hit_test(HitTestBehavior::None)
}

/// Room as wide as a folded column's strip, which lets the pointer through to the strip under it.
fn strip_room() -> Element {
  Column::new()
    .width(Dimension::Px(STRIP_WIDTH))
    .flex_shrink(0.0)
    .hit_test(HitTestBehavior::None)
    .into()
}

/// What a drag or a refusal draws over a column's cards, under the room of its head; the pointer goes through
/// wherever it draws nothing.
fn over_cards(over: Option<Element>) -> Element {
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .hit_test(HitTestBehavior::ContentOnly)
    .child(head_room())
    .child(
      Stack::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .stack_align(StackAlignment::TopStart)
        .hit_test(HitTestBehavior::ContentOnly)
        .with_children(over),
    )
    .into()
}
