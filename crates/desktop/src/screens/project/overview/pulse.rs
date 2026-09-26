//! The project pulse (`C95Z93`): four project metrics (`dZ2Zt`) — runs with an agent at work,
//! runs waiting for the user, runs completed today and today's tokens against the daily project
//! budget.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::tokens::compact_tokens;

use crate::{
  tasks::{
    overview::{Pulse, figure},
    usage::ProjectUsage,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::message_text,
};

/// Pulse row height (`C95Z93`).
const PULSE_HEIGHT: f32 = 94.0;

/// `runner`: whether the runner is online, the agents it reported at work, and the runs its
/// absence interrupted.
pub(super) fn pulse(
  ctx: &Ctx,
  pulse: &Pulse,
  (online, agents_working, lost): (bool, Option<usize>, usize),
  usage: Option<ProjectUsage>,
) -> Element {
  let running = message_text(ctx, &Pulse::running_context(online, agents_working, lost));
  let (tokens, budget) = match usage {
    Some(usage) => (
      Arc::from(compact_tokens(usage.today.get())),
      ctx.t_args(
        "overview.metrics.tokensContext",
        [("budget", compact_tokens(usage.limits.daily_budget().get()).as_str())],
      ),
    ),
    None => (
      ctx.t("statusBar.counts.unknown"),
      ctx.t("overview.metrics.tokensUnknown"),
    ),
  };
  let needs_input = figure(pulse.needs_input);
  let completed = figure(pulse.completed_today);
  let running_figure = figure(pulse.running);
  Row::new()
    .id("overview-pulse")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PULSE_HEIGHT))
    .padding_vertical(ExtraSpacing::Step.role())
    .spacing(SpacingSize::Section)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(metric(
      "overview-running",
      &running_figure,
      &ctx.t("overview.metrics.running"),
      &running,
    ))
    .child(metric(
      "overview-needs-input",
      &needs_input,
      &ctx.t("overview.metrics.needsInput"),
      &ctx.t("overview.metrics.needsInputContext"),
    ))
    .child(metric(
      "overview-completed",
      &completed,
      &ctx.t("overview.metrics.completed"),
      &ctx.t("overview.metrics.completedContext"),
    ))
    .child(metric(
      "overview-tokens",
      &tokens,
      &ctx.t("overview.metrics.tokens"),
      &budget,
    ))
    .into()
}

/// One project metric (`dZ2Zt`): the figure and its label, and what it counts.
fn metric(id: &'static str, value: &str, label: &str, context: &str) -> Element {
  Column::new()
    .id(id)
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .spacing(ExtraSpacing::Inline.role())
        .align_items(Alignment::Center)
        .child(Text::new(value).variant(ExtraTypography::Metric.role()).nowrap())
        .child(Text::new(label).variant(TypographyStyle::FieldLabel).nowrap()),
    )
    .child(
      Text::new(context)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}
