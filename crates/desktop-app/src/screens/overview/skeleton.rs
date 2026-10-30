//! The Overview while the project's tasks are read (`01 · Overview · loading` `ae6PR`): the header, every section's
//! head, *Hold new starts* and the sections' links are real; the counts, *At work*'s slots, the merge queue's state
//! and each section's lines are bones in their own lines' geometry, shown after 300 ms so a fast reading never flashes
//! them, pulsing unless reduced motion is asked for, lower lines fading (`ui::skeleton`). Plan limits come from memory
//! and draw at once. *Reading Kontur's tasks…* (`tasks.loading`) names each section's bones for tools; Delivery reads
//! on its own and its bones say so (`overview.delivery.reading`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  at_work::{ACCOUNT_WIDTH, AGE_WIDTH, DASH, STEP_WIDTH},
  delivery::METER_HEIGHT,
  digest,
  queue::PLACE_WIDTH,
  section::{KEY_WIDTH, LINE_HEIGHT, SIDE_LINE_HEIGHT},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, OverviewText},
  ui::skeleton::{self, Skeleton, bone, fading, line, round_bone},
};

/// The bones' widths in each section, a line each (`ae6PR`): *Waiting for you*'s what and task, *At work*'s title and
/// account, *Queue*'s title and why, *Landed*'s title.
const WAITS: [(f32, f32); 4] = [(200.0, 90.0), (260.0, 90.0), (180.0, 70.0), (230.0, 90.0)];
const AT_WORK: [(f32, f32); 3] = [(200.0, 130.0), (180.0, 110.0), (210.0, 120.0)];
const QUEUE: [(f32, f32); 3] = [(200.0, 150.0), (230.0, 180.0), (250.0, 190.0)];
const LANDED: [f32; 4] = [170.0, 140.0, 190.0, 150.0];
/// A key's bone in its room, a time's, a link's and a step name's.
const KEY: f32 = 40.0;
const TIME: f32 = 20.0;
const LINK: f32 = 52.0;
const STEP: f32 = 56.0;
/// How many dashes a step's bone shows.
const DASHES: usize = 4;
/// *Today · N merged*'s bone, *At work*'s slots' and the merge queue state's.
const TODAY: f32 = 90.0;
const SLOTS: f32 = 110.0;
const QUEUE_STATE: f32 = 116.0;
/// Delivery's modules: a name's bone and its forecast's.
const MODULES: [(f32, f32); 2] = [(100.0, 150.0), (120.0, 150.0)];
const DONE: f32 = 56.0;

/// What names a section's bones for tools: the project's tasks are read.
pub(super) fn label(ctx: &Ctx, project: &str) -> Arc<str> {
  ctx.t_args("tasks.loading", [("project", project)])
}

/// A count's bone beside a section's name, under `<id>-count-reading`.
pub(super) fn count(ctx: &mut Ctx, id: &str, label: &Arc<str>) -> Element {
  Skeleton::new(format!("{id}-count-reading"), label.clone())
    .inline()
    .show(ctx, skeleton::count())
}

/// *At work*'s slots at its head's end.
pub(super) fn slots(ctx: &mut Ctx, label: &Arc<str>) -> Element {
  let bones = line(ctx, TypographyStyle::Description, SLOTS);
  Skeleton::new("overview-at-work-slots-reading", label.clone())
    .inline()
    .show(ctx, bones)
}

/// The merge queue's state after its name.
pub(super) fn queue_state(ctx: &mut Ctx, label: &Arc<str>) -> Element {
  let bones = line(ctx, TypographyStyle::Description, QUEUE_STATE);
  Skeleton::new("overview-merge-queue-reading", label.clone())
    .inline()
    .show(ctx, bones)
}

/// *Waiting for you*'s lines (`TyACW`): a 34 line under a hairline, its mark, what waits, its task, how long and its
/// link.
pub(super) fn waits(ctx: &mut Ctx, label: &Arc<str>) -> Element {
  let lines = WAITS
    .iter()
    .map(|(what, task)| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(digest::LINE_HEIGHT))
        .padding(Padding::horizontal(SpacingSize::Md))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(round_bone(IconSize::Base.size()))
        .child(line(ctx, OverviewText::Line.role(), *what))
        .child(cut(line(ctx, ExtraTypography::MonoKey.role(), *task)))
        .child(line(ctx, ExtraTypography::MonoNote.role(), TIME))
        .child(line(ctx, TypographyStyle::Button, LINK))
    })
    .collect();
  // The digest's lines touch, a hairline between them.
  let bones = Column::new().width(Dimension::Pct(100.0)).with_children(fading(lines));
  Skeleton::new("overview-waits-bones", label.clone()).show(ctx, bones)
}

/// *At work*'s lines (`IJRfV`): the loader, the key, the title, the step's dashes and name, the agent's mark and
/// account, and how long.
pub(super) fn at_work(ctx: &mut Ctx, label: &Arc<str>) -> Element {
  let lines = AT_WORK
    .iter()
    .map(|(title, account)| {
      let step = Row::new()
        .width(Dimension::Px(STEP_WIDTH))
        .flex_shrink(0.0)
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Row::new()
            .spacing(ExtraSpacing::Fine.role())
            .with_children((0..DASHES).map(|_| bone(DASH.0, DASH.1))),
        )
        .child(line(ctx, TypographyStyle::Description, STEP));
      let agent = Row::new()
        .width(Dimension::Px(ACCOUNT_WIDTH))
        .flex_shrink(0.0)
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(round_bone(IconSize::Sm.size()))
        .child(line(ctx, TypographyStyle::Description, *account));
      task_line(ctx, LINE_HEIGHT, (IconSize::Md, *title))
        .child(step)
        .child(agent)
        .child(at_end(ctx, AGE_WIDTH))
    })
    .collect();
  shown(ctx, "overview-at-work-bones", label, lines)
}

/// *Queue*'s lines: the clock, the key, the title, why it waits and its place.
pub(super) fn queue(ctx: &mut Ctx, label: &Arc<str>) -> Element {
  let lines = QUEUE
    .iter()
    .map(|(title, why)| {
      task_line(ctx, LINE_HEIGHT, (IconSize::Md, *title))
        .child(line(ctx, TypographyStyle::Description, *why))
        .child(at_end(ctx, PLACE_WIDTH))
    })
    .collect();
  shown(ctx, "overview-queue-bones", label, lines)
}

/// *Landed*'s runs (`F3wZk`): *Today · N merged*, then each run's mark, key, title and time, cut to the side column.
pub(super) fn landed(ctx: &mut Ctx, label: &Arc<str>) -> Element {
  let today = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(ExtraSpacing::Tight.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .child(line(ctx, ExtraTypography::Note.role(), TODAY));
  let runs = LANDED.iter().map(|title| {
    task_line(ctx, SIDE_LINE_HEIGHT, (IconSize::Base, *title))
      .spacing(ExtraSpacing::Loose.role())
      .child(line(ctx, ExtraTypography::MonoNote.role(), TIME))
  });
  let lines = std::iter::once(today).chain(runs).collect();
  shown(ctx, "overview-landed-bones", label, lines)
}

/// Delivery's modules while their forecasts are read (`YkDqG`): each one's mark, name and tasks, its meter and its
/// forecast.
pub(super) fn delivery(ctx: &mut Ctx) -> Element {
  let modules = MODULES
    .iter()
    .map(|(name, forecast)| {
      let head = Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(round_bone(IconSize::Base.size()))
        .child(line(ctx, OverviewText::Line.role(), *name))
        .child(Spacer::new().flex(1.0))
        .child(line(ctx, ExtraTypography::MonoKey.role(), DONE));
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
        .spacing(ExtraSpacing::Snug.role())
        .child(head)
        .child(bone(1.0, METER_HEIGHT).width(Dimension::Pct(100.0)))
        .child(line(ctx, ExtraTypography::Note.role(), *forecast))
    })
    .collect();
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(fading(modules));
  Skeleton::new("overview-delivery-bones", ctx.t("overview.delivery.reading")).show(ctx, bones)
}

/// A task's line: its mark of `icon`'s size, its key in its room and its title, `title` long, cut to the room left.
fn task_line(ctx: &Ctx, height: f32, (icon, title): (IconSize, f32)) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(round_bone(icon.size()))
    .child(Row::new().width(Dimension::Px(KEY_WIDTH)).flex_shrink(0.0).child(line(
      ctx,
      ExtraTypography::MonoKey.role(),
      KEY,
    )))
    .child(cut(line(ctx, ExtraTypography::Strong.role(), title)))
}

/// A bone that takes the room left and is cut to it, as a title is.
fn cut(bone: Row) -> Row {
  Row::new().flex(1.0).min_width(Dimension::Px(0.0)).clip().child(bone)
}

/// A short time or place at a line's end, in its room of `width`.
fn at_end(ctx: &Ctx, width: f32) -> Row {
  Row::new()
    .width(Dimension::Px(width))
    .flex_shrink(0.0)
    .justify(Justify::End)
    .child(line(ctx, ExtraTypography::MonoNote.role(), TIME))
}

/// `lines` under `id`, 2 apart and the lower ones fading, once the reading took 300 ms.
fn shown(ctx: &mut Ctx, id: &str, label: &Arc<str>, lines: Vec<Row>) -> Element {
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(fading(lines));
  Skeleton::new(id, label.clone()).show(ctx, bones)
}
