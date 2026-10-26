//! The Module chip (ORC-48; `Module` `BAgAS`): the module the task goes in, one of the project's modules or none, saved
//! after the task's details, chosen in a menu with a check on the chosen one. A project without modules shows no chip
//! for a task in none; while the task is queued its module is locked with its details.

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::module::Module;
use orchester_workspace::modules::ModuleChoice;

use super::{
  chips::{ChipState, Look, PropertyChip},
  state::View,
};
use crate::ui::{filter::filter_menu_panel, icon::Icon, menu::MenuItem};

/// The menu's width.
const MENU_WIDTH: f32 = 240.0;

pub fn module_chip(
  ctx: &mut Ctx,
  view: &View,
  (modules, state): (&[Module], &ChipState),
  chosen: &Signal<ModuleChoice>,
) -> Option<Element> {
  if modules.is_empty() && view.module.is_none() {
    return None;
  }
  let name = view
    .module
    .and_then(|id| modules.iter().find(|module| module.id() == id))
    .map(|module| module.name().as_str().to_owned());
  let (look, value) = match name {
    Some(name) => (Look::Set, name.into()),
    None => (Look::Empty, ctx.t("task_form.fields.module.none")),
  };
  let panel = state.is_open().then(|| menu(ctx, state, modules, chosen));
  Some(
    PropertyChip::new(
      "task-form-module",
      Icon::Target,
      (ctx.t("task_form.fields.module.label"), value),
      look,
    )
    .locked(view.locked())
    .render(state, panel),
  )
}

fn menu(ctx: &mut Ctx, state: &ChipState, modules: &[Module], chosen: &Signal<ModuleChoice>) -> Element {
  let current = chosen.get().0;
  let choices = std::iter::once((None, ctx.t("task_form.fields.module.noModule"))).chain(
    modules
      .iter()
      .map(|module| (Some(module.id()), module.name().as_str().into())),
  );
  let rows: Vec<Element> = choices
    .enumerate()
    .map(|(index, (id, label))| {
      let chosen = chosen.clone();
      MenuItem::new(format!("task-form-module-{index}"), label, move || {
        chosen.set(ModuleChoice(id));
      })
      .in_form()
      .chosen(id == current)
      .render(ctx, &state.open)
    })
    .collect();
  filter_menu_panel("task-form-module-menu", MENU_WIDTH)
    .with_children(rows)
    .into()
}
