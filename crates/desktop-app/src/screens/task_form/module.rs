//! The Module section (ORC-48, no frame yet, laid out as `Section · Limits` `WpoKn`): the module the task goes in,
//! one of the project's modules or none, saved after the task's details. A project without modules shows no section
//! for a task in none; while the task is queued its module is locked with its details.

use lurq::{app::ctx::Ctx, components::SelectOption, core::Signal, node::Element};
use orchester_core::module::Module;
use orchester_workspace::modules::ModuleChoice;

use super::state::View;
use crate::ui::{
  field::SelectField,
  form_section::{Label, Place, section, side_row},
};

pub fn module_section(
  ctx: &mut Ctx,
  view: &View,
  modules: &[Module],
  chosen: &Signal<ModuleChoice>,
) -> Option<Element> {
  if modules.is_empty() && view.module.is_none() {
    return None;
  }
  let mut options = vec![SelectOption::new(
    ModuleChoice(None),
    ctx.t("task_form.fields.module.none"),
  )];
  options.extend(
    modules
      .iter()
      .map(|module| SelectOption::new(ModuleChoice(Some(module.id())), module.name().as_str())),
  );
  let select = SelectField::new(
    "task-form-module-input",
    ctx.t("task_form.fields.module.label"),
    chosen.clone(),
    options,
  );
  let select = if view.locked() { select.locked() } else { select };
  let row = side_row(
    Label::new("task-form-module", ctx.t("task_form.fields.module.label"))
      .description(ctx.t("task_form.fields.module.description")),
    select.render(ctx),
    Place::Last,
    true,
  );
  Some(section("task-form-module", &ctx.t("task_form.sections.module"), None, vec![row]).into())
}
