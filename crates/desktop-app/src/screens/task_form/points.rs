//! The Story points section (ORC-49; no design frame yet, built like the Limits section `WpoKn` with the pipeline
//! select `ZIliW`): *Automatic*, Orchester's estimate, or a person's 1, 2, 3, 5, 8, 13 or 21, which automation never
//! changes. Under it, while the task has Orchester's estimate, its points and why.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, SelectOption, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::task::estimate::{EstimateSource, StoryEstimate, StoryPoints};

use super::state::View;
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    field::{FieldInput, SelectField},
    form_section::{Label, Place, section, side_row},
    icon::Icon,
  },
};

/// The select's width, as the pipeline select's (`Control` `ZIliW`).
const SELECT_WIDTH: f32 = 240.0;

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

pub fn points_section(ctx: &mut Ctx, view: &View, pick: &Signal<Points>) -> Element {
  let label = Label::new("task-form-points", ctx.t("task_form.fields.points.label"))
    .description(ctx.t("task_form.fields.points.description"));
  let name = ctx.t("task_form.fields.points.label");
  let shown = |ctx: &Ctx, points: Points| match points.0 {
    Some(points) => points_text(ctx, points),
    None => ctx.t("task_form.fields.points.automatic"),
  };
  let control = if view.locked() {
    FieldInput::new(
      "task-form-points-input",
      name,
      Signal::new(shown(ctx, Points(view.values.points)).to_string()),
    )
    .locked()
    .trailing(Icon::Lock)
    .render(ctx)
  } else {
    let options = std::iter::once(Points(None))
      .chain(StoryPoints::all().map(|points| Points(Some(points))))
      .map(|points| SelectOption::new(points, shown(ctx, points)))
      .collect();
    SelectField::new("task-form-points-input", name, pick.clone(), options)
      .width(Dimension::Px(SELECT_WIDTH))
      .render(ctx)
  };
  let estimate = view
    .subject
    .task()
    .and_then(|task| task.estimate())
    .filter(|estimate| estimate.source() == EstimateSource::Automation && view.values.points.is_none())
    .map(|estimate| estimated(ctx, estimate));
  let control = Column::new()
    .width(Dimension::Px(SELECT_WIDTH))
    .spacing(ExtraSpacing::Snug.role())
    .child(control)
    .with_children(estimate);
  section(
    "task-form-points-section",
    &ctx.t("task_form.sections.points"),
    None,
    vec![side_row(label, Element::from(control), Place::Last, false)],
  )
  .into()
}

/// Orchester's estimate and its reason, under the select.
fn estimated(ctx: &Ctx, estimate: &StoryEstimate) -> Text {
  let points = points_text(ctx, estimate.points()).to_string();
  let words = match estimate.reason() {
    Some(reason) => ctx.t_args(
      "task_form.fields.points.estimatedWhy",
      [("points", points.as_str()), ("reason", reason)],
    ),
    None => ctx.t_args("task_form.fields.points.estimated", [("points", points.as_str())]),
  };
  Text::new(&words)
    .id("task-form-points-estimate")
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
}
