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
  node::{Element, dimension::Dimension},
};
use orchester_core::tokens::compact_tokens;

use crate::{
  tasks::{
    overview::{Pulse, figure},
    usage::ProjectUsage,
  },
  ui::message_text,
};

/// `runner`: whether the runner is online, the agents it reported at work, and the runs its
/// absence interrupted.
pub(super) fn pulse(
  ctx: &Ctx,
  pulse: &Pulse,
  (online, agents_working, lost): (bool, Option<usize>, usize),
  usage: Option<ProjectUsage>,
  in_review: usize,
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
    .spacing(SpacingSize::Sm)
    .child(
      Column::new()
        .flex(1.0)
        .padding(SpacingSize::Md)
        .spacing(SpacingSize::Sm)
        .background(PaletteColor::SurfaceInput)
        .border_inside(BorderSize::Sm, PaletteColor::Border)
        .rounded(lurq::app::theme::RadiusSize::Md)
        .child(summary_metric(
          "overview-running",
          &running_figure,
          &ctx.t("overview.metrics.running"),
        ))
        .child(summary_metric(
          "overview-needs-input",
          &needs_input,
          &ctx.t("overview.metrics.needsInput"),
        ))
        .child(Text::new(&running).variant(TypographyStyle::Caption)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .padding(SpacingSize::Md)
        .spacing(SpacingSize::Sm)
        .background(PaletteColor::SurfaceInput)
        .border_inside(BorderSize::Sm, PaletteColor::Border)
        .rounded(lurq::app::theme::RadiusSize::Md)
        .child(summary_metric(
          "overview-in-review",
          &in_review.to_string(),
          &ctx.t("tasks.workflow.inReview"),
        ))
        .child(summary_metric(
          "overview-completed",
          &completed,
          &ctx.t("overview.metrics.completed"),
        ))
        .child(summary_metric(
          "overview-tokens",
          &tokens,
          &ctx.t("overview.metrics.tokens"),
        ))
        .child(Text::new(&budget).variant(TypographyStyle::Caption)),
    )
    .into()
}

fn summary_metric(id: &'static str, value: &str, label: &str) -> Element {
  Row::new()
    .spacing(SpacingSize::Sm)
    .id(id)
    .child(Text::new(value).variant(TypographyStyle::Body))
    .child(Text::new(label).variant(TypographyStyle::Body))
    .into()
}
