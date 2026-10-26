//! The Points chip (ORC-49; `Points` `qbkXj`): *Automatic*, Orchester's estimate, or a person's 1, 2, 3, 5, 8, 13 or
//! 21, which automation never changes, chosen in a menu with a check on the chosen one. Under the choices, while the
//! task has Orchester's estimate, its points and why. Automatic is the empty chip (*Points · Auto*); a person's points
//! read as a set chip.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::task::estimate::{EstimateSource, StoryEstimate, StoryPoints};

use super::{
  chips::{ChipState, Look, PropertyChip},
  state::View,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{filter::filter_menu_panel, icon::Icon, menu::MenuItem},
};

/// The menu's width.
const MENU_WIDTH: f32 = 260.0;

/// The person's story points; `None`: Automatic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Points(pub Option<StoryPoints>);

impl lurq::app::component::DevtoolsInspectable for Points {}

/// `points` in words: *1 point*, *5 points*.
pub fn points_text(ctx: &Ctx, points: StoryPoints) -> Arc<str> {
  if points.get() == 1 {
    ctx.t("task_form.fields.points.one")
  } else {
    ctx.t_args("task_form.fields.points.other", [("count", points.get().to_string())])
  }
}

pub fn points_chip(ctx: &mut Ctx, view: &View, state: &ChipState, pick: &Signal<Points>) -> Element {
  let (look, value) = match view.values.points {
    Some(points) => (Look::Set, points_text(ctx, points)),
    None => (Look::Empty, ctx.t("task_form.fields.points.auto")),
  };
  let panel = state.is_open().then(|| menu(ctx, view, state, pick));
  PropertyChip::new(
    "task-form-points",
    Icon::Hash,
    (ctx.t("task_form.fields.points.label"), value),
    look,
  )
  .locked(view.locked())
  .render(state, panel)
}

fn menu(ctx: &mut Ctx, view: &View, state: &ChipState, pick: &Signal<Points>) -> Element {
  let chosen = view.values.points;
  let rows: Vec<Element> = std::iter::once(None)
    .chain(StoryPoints::all().map(Some))
    .map(|points| {
      let label = points.map_or_else(
        || ctx.t("task_form.fields.points.automatic"),
        |points| points_text(ctx, points),
      );
      let id = points.map_or_else(|| "auto".to_owned(), |points| points.get().to_string());
      let pick = pick.clone();
      let item = MenuItem::new(format!("task-form-points-{id}"), label, move || {
        pick.set(Points(points))
      })
      .in_form()
      .chosen(points == chosen);
      let item = if points.is_none() {
        item.detail(ctx.t("task_form.fields.points.automaticDetail"))
      } else {
        item
      };
      item.render(ctx, &state.open)
    })
    .collect();
  let estimate = view
    .subject
    .task()
    .and_then(|task| task.estimate())
    .filter(|estimate| estimate.source() == EstimateSource::Automation && chosen.is_none())
    .map(|estimate| estimated(ctx, estimate));
  filter_menu_panel("task-form-points-menu", MENU_WIDTH)
    .with_children(rows)
    .with_children(estimate)
    .into()
}

/// Orchester's estimate and its reason, under the choices.
fn estimated(ctx: &Ctx, estimate: &StoryEstimate) -> Column {
  let points = points_text(ctx, estimate.points()).to_string();
  let words = match estimate.reason() {
    Some(reason) => ctx.t_args(
      "task_form.fields.points.estimatedWhy",
      [("points", points.as_str()), ("reason", reason)],
    ),
    None => ctx.t_args("task_form.fields.points.estimated", [("points", points.as_str())]),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .child(
      Text::new(&words)
        .id("task-form-points-estimate")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
}
