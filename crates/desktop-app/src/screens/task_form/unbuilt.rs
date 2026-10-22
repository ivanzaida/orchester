//! The form's section whose feature is not built yet, in place and visibly unavailable with why: Instructions
//! (`Section · Instructions` `y4wsI`, previewing what the agents are told). Context holds the task's sources
//! (`super::sources`).

use lurq::{app::ctx::Ctx, node::Element};

use crate::ui::form_section::{Label, Place, not_built, section, side_row};

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
