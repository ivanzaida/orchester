//! The form's sections whose features are not built yet, in place and visibly unavailable with why:
//! Context (`Section · Context` `J5IB3`, attaching files, links and notes) and Instructions (`Section ·
//! Instructions` `y4wsI`, previewing what the agents are told).

use lurq::{app::ctx::Ctx, node::Element};

use super::sections::{Label, Place, not_built, section, side_row};

pub fn context_section(ctx: &Ctx) -> Element {
  let label = Label::new("task-form-context", ctx.t("task_form.fields.context.label"))
    .description(ctx.t("task_form.fields.context.description"));
  let control = not_built(
    "task-form-attach-context",
    &ctx.t("task_form.fields.context.action"),
    &ctx.t("task_form.fields.context.reason"),
  );
  let rows = vec![side_row(label, control, Place::Last, false)];
  section(
    "task-form-context-section",
    &ctx.t("task_form.sections.context"),
    None,
    rows,
  )
  .into()
}

pub fn instructions_section(ctx: &Ctx) -> Element {
  let label = Label::new("task-form-instructions", ctx.t("task_form.fields.instructions.label"))
    .description(ctx.t("task_form.fields.instructions.description"));
  let control = not_built(
    "task-form-preview-instructions",
    &ctx.t("task_form.fields.instructions.action"),
    &ctx.t("task_form.fields.instructions.reason"),
  );
  let rows = vec![side_row(label, control, Place::Last, false)];
  section(
    "task-form-instructions-section",
    &ctx.t("task_form.sections.instructions"),
    None,
    rows,
  )
  .into()
}
