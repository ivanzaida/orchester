//! The list while it is read (`04 · Review · reading` `M1LAx`): *Waiting for you*'s header over as many row bones as
//! the sidebar's count says, in the geometry of the rows that replace them, so nothing above them moves when the rows
//! arrive; with none waiting the page is `03` at once, never after bones (the page draws it, not this module). The
//! lower groups are not drawn until they are read. The bones show from the first frame, as the page's head does.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{
  items::Group,
  line::{GROUP_HEIGHT, group_id, label_key},
  row::ROW_HEIGHT,
};
use crate::{
  theme::{ExtraSpacing, IconSize, TasksSpacing},
  ui::{
    icon::{Icon, icon},
    skeleton::{Skeleton, bone, round_bone},
  },
};

/// A row's bones (`Row · reading` `OluKF`): the status dot, the key, the title, the state, the counts and the time.
const STATUS: f32 = 14.0;
const KEY_CELL: f32 = 52.0;
const KEY: (f32, f32) = (44.0, 10.0);
const STATE: (f32, f32) = (120.0, 10.0);
const COUNTS: (f32, f32) = (80.0, 10.0);
const TIME_CELL: f32 = 36.0;
const TIME: (f32, f32) = (30.0, 10.0);
const TITLE_HEIGHT: f32 = 10.0;
/// The titles' bone widths, one after the other, as `04` draws three rows.
const TITLES: [f32; 3] = [220.0, 180.0, 250.0];
/// The count's bone in the group's header (`I4QNRT`).
const COUNT: (f32, f32) = (14.0, 8.0);

/// The page body while the list is read: with `waiting` known, *Waiting for you*'s header and its bones; unknown (the
/// project's tasks are read first), the header alone.
pub(super) fn reading(ctx: &mut Ctx, waiting: Option<usize>) -> Element {
  let label = ctx.t("review.loading");
  let header = header(ctx);
  let rows = (0..waiting.unwrap_or(0)).map(|index| row(TITLES[index % TITLES.len()]));
  let bones = Column::new()
    .id("review-loading-rows")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().right(TasksSpacing::Sides.role()))
    .spacing(ExtraSpacing::Tight.role())
    .child(header)
    .with_children(rows);
  Skeleton::new("review-loading", label).show(ctx, bones)
}

/// *Waiting for you*'s header while it is read: its chevron and name, and a bone where the count comes.
fn header(ctx: &Ctx) -> Row {
  let id = group_id(Group::Waiting);
  Row::new()
    .id(format!("{id}-reading"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GROUP_HEIGHT))
    .padding(
      Padding::new()
        .top(TasksSpacing::GroupTop.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t(label_key(Group::Waiting)))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(bone(COUNT.0, COUNT.1))
}

/// A row's bones in the row's cells.
fn row(title: f32) -> Row {
  let cell = |width: f32, justify: Justify| {
    Row::new()
      .width(Dimension::Px(width))
      .flex_shrink(0.0)
      .justify(justify)
      .align_items(Alignment::Center)
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(round_bone(STATUS))
    .child(cell(KEY_CELL, Justify::Start).child(bone(KEY.0, KEY.1)))
    .child(
      Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(bone(title, TITLE_HEIGHT)),
    )
    .child(bone(STATE.0, STATE.1))
    .child(bone(COUNTS.0, COUNTS.1))
    .child(cell(TIME_CELL, Justify::End).child(bone(TIME.0, TIME.1)))
}
