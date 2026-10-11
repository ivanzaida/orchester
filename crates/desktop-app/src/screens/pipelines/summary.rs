//! Version and step metadata of the pipeline summary (c24Tv / Z26ZFs).
use lurq::app::ctx::Ctx;

use super::state::Editing;

pub fn published(ctx: &Ctx, editing: &Editing) -> std::sync::Arc<str> {
  editing.opened.latest.as_ref().map_or_else(
    || ctx.t("pipelines.never"),
    |v| {
      ctx.t_args(
        "pipelines.publishedAt",
        [
          ("version", v.number.get().to_string()),
          ("age", super::library::age(ctx, v.at).to_string()),
        ],
      )
    },
  )
}
pub fn draft(ctx: &Ctx, editing: &Editing) -> std::sync::Arc<str> {
  let number = orchester_workspace::pipelines::write::next_version(&editing.opened)
    .map_or_else(String::new, |v| v.get().to_string());
  if editing.opened.draft_saved_at.is_none() && !editing.dirty() {
    return ctx.t_args("pipelines.noDraftNext", [("version", number)]);
  }
  if editing.edit.steps().is_empty() {
    return ctx.t_args("pipelines.draftNoSteps", [("version", number)]);
  }
  let Some(latest) = &editing.opened.latest else {
    return ctx.t_args("pipelines.draftVersion", [("version", number)]);
  };
  let Ok(body) = editing.edit.validate() else {
    return ctx.t("pipelines.unsaved");
  };
  let changes = orchester_core::resource::changes(&latest.body, &body).len();
  ctx.t_args(
    if changes == 1 {
      "pipelines.draftOneChange"
    } else {
      "pipelines.draftChanges"
    },
    [
      ("version", number),
      ("count", changes.to_string()),
      ("base", latest.number.get().to_string()),
    ],
  )
}
pub fn steps(ctx: &Ctx, shown: &orchester_core::resource::PipelineEdit) -> String {
  let gates = shown.steps().iter().filter(|s| s.approval_gate).count();
  if shown.steps().is_empty() {
    return shown.steps().len().to_string();
  }
  ctx
    .t_args(
      if gates == 1 {
        "pipelines.stepsOneGate"
      } else {
        "pipelines.stepsGates"
      },
      [("count", shown.steps().len().to_string()), ("gates", gates.to_string())],
    )
    .to_string()
}
