//! The Priority chip (`Priority` `uZEkZ`, set `iEEC4`): the task's priority, a part of its organization, chosen in a
//! menu of today's priorities (`ui::status::priority_look`) with a check on the chosen one. No priority is the empty
//! chip; another reads its word with its icon.

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::task::organization::Priority;

use super::{
  chips::{ChipState, Look, PropertyChip},
  state::View,
};
use crate::ui::{filter::filter_menu_panel, icon::Icon, menu::MenuItem, status::priority_look};

/// The menu's width.
const MENU_WIDTH: f32 = 220.0;

/// The task's priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriorityPick(pub Priority);

impl lurq::app::component::DevtoolsInspectable for PriorityPick {}

pub fn priority_chip(ctx: &mut Ctx, view: &View, state: &ChipState, pick: &Signal<PriorityPick>) -> Element {
  let priority = view.values.priority;
  let (look, glyph, value) = match priority {
    Priority::None => (Look::Empty, Icon::Signal, ctx.t("task_form.fields.priority.none")),
    priority => {
      let (glyph, key, _) = priority_look(priority);
      (Look::Set, glyph, ctx.t(key))
    }
  };
  let panel = state.is_open().then(|| menu(ctx, state, pick, priority));
  PropertyChip::new(
    "task-form-priority",
    glyph,
    (ctx.t("task_form.fields.priority.label"), value),
    look,
  )
  .locked(view.locked())
  .render(state, panel)
}

fn menu(ctx: &mut Ctx, state: &ChipState, pick: &Signal<PriorityPick>, chosen: Priority) -> Element {
  let rows: Vec<Element> = Priority::ALL
    .into_iter()
    .map(|priority| {
      let (glyph, key, _) = priority_look(priority);
      let pick = pick.clone();
      MenuItem::new(format!("task-form-priority-{priority:?}"), ctx.t(key), move || {
        pick.set(PriorityPick(priority));
      })
      .icon(glyph)
      .in_form()
      .chosen(priority == chosen)
      .render(ctx, &state.open)
    })
    .collect();
  filter_menu_panel("task-form-priority-menu", MENU_WIDTH)
    .with_children(rows)
    .into()
}
