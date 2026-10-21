//! The Merging section (ORC-69): *Don't merge automatically*, a part of the task's organization, in a form row with
//! its switch at the end (`Form row` `XaNIz`, as Settings' switches). On, the Orchestrator never merges the task's runs
//! on its own, even while the person lets it merge finished runs; the turn a run's success starts only tells the
//! person. It is saved with the form, as its other fields are.

use lurq::{app::ctx::Ctx, core::Signal, node::Element};

use super::state::View;
use crate::ui::{
  form_section::{Label, Place, section, side_row},
  toggle::{Toggle, ToggleKind},
};

pub fn merging_section(ctx: &mut Ctx, view: &View, field: &Signal<bool>) -> Element {
  let label = ctx.t("task_form.fields.noAutomaticMerge.label");
  let switch = Toggle::new(
    "task-form-no-automatic-merge",
    ToggleKind::Switch,
    view.values.no_automatic_merge,
    label.clone(),
  )
  .hide_label();
  // While a save or a start is under way, or the task is queued, the switch stays as it is, as every field does.
  let switch = if view.locked() {
    switch
  } else {
    let field = field.clone();
    switch.on_change(move |on| field.set(on))
  };
  let row = side_row(
    Label::new("task-form-no-automatic-merge", label).description(ctx.t("task_form.fields.noAutomaticMerge.description")),
    switch,
    Place::Last,
    false,
  );
  section("task-form-merging", &ctx.t("task_form.sections.merging"), None, vec![row]).into()
}
