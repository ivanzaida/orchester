//! *…* (`More` `LAiJr`) and its popover: *Don't merge automatically* (ORC-69), a part of the task's organization, as a
//! switch with its description. On, the Orchestrator never merges the task's runs on its own, even while the person
//! lets it merge finished runs; the turn a run's success starts only tells the person. It is saved with the form, as
//! its other fields are. A shared project's server does not keep it, so a shared project's form has no *…*, and the
//! task writer refuses the flag there.

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};

use super::{
  chips::{ChipState, more_chip},
  state::View,
};
use crate::{
  theme::ExtraSpacing,
  ui::{
    popover::popover_panel,
    toggle::{Toggle, ToggleKind},
  },
};

/// The popover's width.
const WIDTH: f32 = 340.0;

pub fn more(ctx: &mut Ctx, view: &View, (shared, state): (bool, &ChipState), field: &Signal<bool>) -> Option<Element> {
  if shared {
    return None;
  }
  let on = view.values.no_automatic_merge;
  let panel = state.is_open().then(|| panel(ctx, view, field));
  Some(more_chip(ctx, state, on, panel))
}

fn panel(ctx: &Ctx, view: &View, field: &Signal<bool>) -> Element {
  let label = ctx.t("task_form.fields.noAutomaticMerge.label");
  let switch = Toggle::new(
    "task-form-no-automatic-merge",
    ToggleKind::Switch,
    view.values.no_automatic_merge,
    label,
  );
  // While a save or a start is under way, or the task is queued, the switch stays as it is, as every field does.
  let switch = if view.locked() {
    switch
  } else {
    let field = field.clone();
    switch.on_change(move |on| field.set(on))
  };
  popover_panel("task-form-more-popover", WIDTH)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(switch)
        .child(
          Text::new(&ctx.t("task_form.fields.noAutomaticMerge.description"))
            .id("task-form-no-automatic-merge-description")
            .width(Dimension::Pct(100.0))
            .variant(TypographyStyle::Caption),
        ),
    )
    .into()
}
