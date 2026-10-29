//! The Overview while the project's tasks are read (04 `A3qRTI`, and `01 · Overview · loading` `ae6PR`): the header,
//! the sections' heads and their actions are real; each section's rows are bones in their own geometry (a status dot,
//! the key, the title, and what is at the row's end), shown after 300 ms so a fast reading never flashes them, pulsing
//! unless reduced motion is asked for, lower rows fading (`ui::skeleton`). Plan limits come from memory and draw at
//! once. *Reading Kontur's tasks…* (`tasks.loading`) names each section's bones for tools.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Spacer},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  project::ShownProject,
  ui::skeleton::{Skeleton, bone, fading, round_bone},
};

/// A row's bones: the status dot, the key and the end (`Row bone` in 04).
const DOT: f32 = 14.0;
const KEY: (f32, f32) = (40.0, 9.0);
const TITLE_HEIGHT: f32 = 9.0;
const END: (f32, f32) = (52.0, 9.0);

/// Bones for rows of `height` under `id`, their titles as wide as `titles` says in turn, named for `project`.
pub(super) fn bones(ctx: &mut Ctx, project: &ShownProject, (id, height): (&str, f32), titles: &[f32]) -> Element {
  let rows = titles
    .iter()
    .map(|title| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(height))
        .padding(Padding::horizontal(SpacingSize::Sm))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(round_bone(DOT))
        .child(bone(KEY.0, KEY.1))
        .child(bone(*title, TITLE_HEIGHT))
        .child(Spacer::new().flex(1.0))
        .child(bone(END.0, END.1))
    })
    .collect();
  let label = ctx.t_args("tasks.loading", [("project", project.open.project.name().as_str())]);
  Skeleton::new(id, label).show(
    ctx,
    Column::new().width(Dimension::Pct(100.0)).with_children(fading(rows)),
  )
}
