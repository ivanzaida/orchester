//! The Summary before the task's first run (`Summary · empty before first run` `Pl3KV` in `Xxrdy`), at the top of its
//! column over the brief: no run has progress, results or history to show, so a 32 `workflow` glyph, *No runs yet* and
//! what the Summary will hold, then *Start run*, 16 apart. Start run is the header's control for the first run
//! (`header::controls::first_start`), sharing its request: unavailable with the same reason, which shows under it; a
//! draft that misses details has none to offer, and says so.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  screens::task::{
    TaskContext,
    header::controls::{Controls, RunControls, first_start},
  },
  theme::TaskText,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, glyph},
  },
};

/// The widest the explanation runs (`hD881`).
const TEXT_WIDTH: f32 = 460.0;

pub fn empty(ctx: &mut Ctx, task: &TaskContext) -> Element {
  let (start, reason) = start(ctx, task);
  let reason = reason.map(|reason| {
    Text::new(&reason)
      .id("task-empty-start-reason")
      .variant(TypographyStyle::Caption)
      .text_align(TextAlign::Center)
      .max_width(Dimension::Px(TEXT_WIDTH))
      .describe("role", "unavailable-reason")
  });
  Column::new()
    .id("task-summary-empty")
    .describe("state", "no-runs")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Xl))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .child(
      Text::new(&glyph(Icon::Workflow))
        .id("task-summary-empty-glyph")
        .variant(TaskText::EmptyGlyph.role()),
    )
    .child(
      Text::new(&ctx.t("summary.empty.title"))
        .id("task-summary-empty-title")
        .variant(TaskText::EmptyTitle.role())
        .text_align(TextAlign::Center),
    )
    .child(
      Text::new(&ctx.t("summary.empty.text"))
        .id("task-summary-empty-text")
        .variant(TaskText::EmptyText.role())
        .text_align(TextAlign::Center)
        .width(Dimension::Px(TEXT_WIDTH))
        .max_width(Dimension::Pct(100.0)),
    )
    .child(start)
    .with_children(reason)
    .into()
}

/// Start run, and why it is unavailable when it is.
fn start(ctx: &mut Ctx, task: &TaskContext) -> (ActionButton, Option<Arc<str>>) {
  let run_controls = RunControls::of(ctx);
  let controls = Controls {
    detail: &task.detail,
    online: task.online,
    action: &run_controls.action,
    pending: run_controls.pending(),
    requested: &run_controls.requested,
    approval_waits: false,
  };
  if let Some(control) = first_start(ctx, &controls, "task-empty-start") {
    let reason = control.reason().map(|reason| reason.full.clone());
    return (control.button(), reason);
  }
  let missing = ctx.t("summary.empty.missing");
  let button = ActionButton::new("task-empty-start", ButtonKind::Primary, ctx.t("task.controls.start"))
    .icon(Icon::Play)
    .unavailable(missing.clone());
  (button, Some(missing))
}
