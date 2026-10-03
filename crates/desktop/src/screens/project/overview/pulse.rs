//! The two ORC-39 summary panels (DtpHJ) and retained execution details.

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
pub(super) fn pulse(ctx: &Ctx, pulse: &Pulse, in_review: usize) -> Element {
  Row::new()
    .id("overview-pulse")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      summary_panel()
        .child(count_line(
          ctx,
          "overview-running",
          "overview.summary.running",
          pulse.running,
          TypographyStyle::Body,
        ))
        .child(count_line(
          ctx,
          "overview-needs-input",
          if pulse.needs_input == 1 {
            "overview.summary.needsInputOne"
          } else {
            "overview.summary.needsInputOther"
          },
          pulse.needs_input,
          TypographyStyle::Caption,
        )),
    )
    .child(
      summary_panel()
        .child(count_line(
          ctx,
          "overview-in-review",
          "overview.summary.inReview",
          in_review,
          TypographyStyle::Body,
        ))
        .child(count_line(
          ctx,
          "overview-completed",
          "overview.summary.completed",
          pulse.completed_today,
          TypographyStyle::Caption,
        )),
    )
    .into()
}

fn summary_panel() -> Column {
  Column::new()
    .flex(1.0)
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceInput)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(lurq::app::theme::RadiusSize::Md)
}

fn count_line(ctx: &Ctx, id: &'static str, key: &'static str, count: usize, typography: TypographyStyle) -> Text {
  Text::new(&ctx.t_args(key, [("count", figure(count).as_str())]))
    .id(id)
    .variant(typography)
}

/// Runner availability, reported agent count, token usage and budget remain accessible.
pub(super) fn execution_context(
  ctx: &Ctx,
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
  Column::new()
    .spacing(SpacingSize::Sm)
    .child(Text::new(&running).variant(TypographyStyle::Caption))
    .child(
      Row::new()
        .id("overview-tokens")
        .spacing(SpacingSize::Sm)
        .child(Text::new(&tokens).variant(TypographyStyle::Body))
        .child(Text::new(&ctx.t("overview.metrics.tokens")).variant(TypographyStyle::Body)),
    )
    .child(Text::new(&budget).variant(TypographyStyle::Caption))
    .into()
}
