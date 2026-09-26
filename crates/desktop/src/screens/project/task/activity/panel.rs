//! The invocation panel beside the transcript (`n0cEOG`): the step and attempt, agent and model
//! and start of the latest agent launch, its reported tokens, the run's total, and the agent's
//! permissions. Token counts are what the CLI reported; cached input is not counted.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::{Column, Row, Spacer, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::run::{Invocation, compact_tokens};

use super::super::summary::permissions;
use crate::{
  display,
  tasks::detail::RunView,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::property::property,
};

/// Panel width (`n0cEOG`).
const PANEL_WIDTH: f32 = 320.0;

pub(super) fn invocation_panel(ctx: &Ctx, run: &RunView) -> Element {
  let panel = Column::new()
    .id("activity-invocation")
    .width(Dimension::Px(PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Step.role())
    .spacing(ExtraSpacing::Group.role())
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("runs.activity.panel.title")).variant(ExtraTypography::SectionHeading.role()));
  let Some(invocation) = run.invocations.last() else {
    return panel
      .child(Text::new(&ctx.t("runs.activity.panel.none")).variant(ExtraTypography::Detail.role()))
      .into();
  };
  panel
    .with_children(details(ctx, run, invocation))
    .child(divider())
    .with_children(tokens(ctx, run, invocation))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("runs.activity.panel.note")).variant(ExtraTypography::Hint.role()))
    .into()
}

fn details(ctx: &Ctx, run: &RunView, invocation: &Invocation) -> Vec<Element> {
  let Some(step) = run.step_of(invocation.execution) else {
    return Vec::new();
  };
  let attempt = run
    .run
    .attempts()
    .iter()
    .find(|attempt| attempt.execution(invocation.execution).is_some())
    .map_or_else(String::new, |attempt| attempt.number().to_string());
  let number = format!("{:02}", step.number);
  let step_value = ctx.t_args(
    "runs.activity.panel.stepValue",
    [
      ("number", number.as_str()),
      ("step", &*step.name),
      ("attempt", attempt.as_str()),
    ],
  );
  let agent = ctx.t_args(
    "runs.activity.panel.agentValue",
    [
      ("agent", step.agent.as_deref().unwrap_or_default()),
      ("model", invocation.model.as_str()),
    ],
  );
  let started = display::clock(invocation.started_at);
  let invocation_number = invocation.number.to_string();
  vec![
    property(&ctx.t("runs.activity.panel.step"), &step_value),
    property(&ctx.t("runs.activity.panel.agent"), &agent),
    property(&ctx.t("runs.activity.panel.invocation"), &invocation_number),
    property(&ctx.t("runs.activity.panel.started"), &started),
    property(&ctx.t("runs.summary.permissions"), &permissions(ctx, step)),
  ]
}

fn tokens(ctx: &Ctx, run: &RunView, invocation: &Invocation) -> Vec<Element> {
  let usage = invocation.usage;
  let count = |value: u64| compact_tokens(value);
  let counted = ctx.t_args("runs.tokens", [("count", count(usage.counted()).as_str())]);
  let cached = ctx.t_args("runs.cachedValue", [("count", count(usage.cached_input).as_str())]);
  let total = ctx.t_args("runs.tokens", [("count", count(run.usage().counted()).as_str())]);
  vec![
    property(&ctx.t("runs.activity.panel.input"), &count(usage.counted_input())),
    property(&ctx.t("runs.activity.panel.output"), &count(usage.output)),
    property(&ctx.t("runs.cached"), &cached),
    property(&ctx.t("runs.activity.panel.counted"), &counted),
    property(&ctx.t("runs.activity.panel.runTotal"), &total),
  ]
}

fn divider() -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .into()
}
