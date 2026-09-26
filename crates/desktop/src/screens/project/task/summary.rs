//! The Summary tab (`Nlcnu` in 02.10 `RD4pc`): what the run is doing in a notice, for a run
//! stopped at a limit the panel that raises it (02.L `ZUYM0`), the pipeline's steps with their
//! state (`mBkeJ` rows), the latest agent launch, and the run's pinned configuration, limits and
//! usage in the "This run" panel (`F5Lsi`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::stream::InvocationOutcome,
  limits::ProjectLimits,
  resource::RepositoryAccess,
  run::{PauseReason, RunLimit, RunStatus, StepStatus},
  time::Timestamp,
  tokens::compact_tokens,
};

use super::{
  heading::{agent_of, step_state},
  limit_panel::{RaiseLimitPanel, RaiseLimitProps},
  notice::notice,
};
use crate::{
  budget::whole_minutes,
  display,
  tasks::detail::{RunView, StepView, TaskDetail},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    property::{ValueStyle, property, styled_property},
  },
};

/// "This run" panel width, step row height and its glyph tile (`F5Lsi`, `NBmWz`, `tWFDE`).
const PANEL_WIDTH: f32 = 336.0;
const STEP_ROW_HEIGHT: f32 = 64.0;
const GLYPH_SIZE: f32 = 28.0;

pub(super) fn summary(ctx: &mut Ctx, detail: &TaskDetail, limits: Option<ProjectLimits>) -> Element {
  let raise = detail.run.as_ref().and_then(|run| raise_panel(ctx, run, limits));
  let mut progress = Column::new()
    .id("task-progress")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Form.role())
    .child(notice(ctx, detail))
    .with_children(raise)
    .child(Text::new(&ctx.t("runs.summary.execution")).variant(ExtraTypography::SectionHeading.role()));
  if let Some(run) = &detail.run {
    progress = progress
      .with_children(run.steps.iter().map(|step| step_row(ctx, step, run.run.status())))
      .child(Spacer::new().flex(1.0))
      .with_children(latest(ctx, run));
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(progress)
    .with_children(detail.run.as_ref().map(|run| this_run(ctx, run)))
    .into()
}

/// One step and its state (`mBkeJ` as in `NBmWz`).
fn step_row(ctx: &Ctx, step: &StepView, run: &RunStatus) -> Element {
  // A step a paused run stopped in is not running (`tSDux` in 02.A).
  let (glyph, state) = match (run, step.status) {
    (RunStatus::Paused(_), Some(StepStatus::Running)) => (Icon::Pause, "runs.step.paused"),
    (RunStatus::Cancelled, Some(StepStatus::Skipped)) => (Icon::Square, "runs.step.cancelled"),
    _ => step_state(step.status),
  };
  let state = ctx.t(state);
  let detail = if step.agent.is_some() {
    ctx.t_args(
      "runs.summary.stepDetail",
      [("agent", agent_of(step)), ("state", &state)],
    )
  } else {
    state
  };
  let tile = Row::new()
    .size(Dimension::Px(GLYPH_SIZE), Dimension::Px(GLYPH_SIZE))
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::Event, PaletteColor::TextSecondary));
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STEP_ROW_HEIGHT))
    .spacing(ExtraSpacing::Stack.role())
    .describe("step", step.name.to_string())
    .describe("state", format!("{:?}", step.status))
    .child(tile)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&step.name)
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextPrimary),
        )
        .child(Text::new(&detail).variant(TypographyStyle::Caption)),
    )
    .into()
}

/// Raise the limit to continue (`ZUYM0`), for a run stopped at a limit.
fn raise_panel(ctx: &mut Ctx, run: &RunView, limits: Option<ProjectLimits>) -> Option<Element> {
  // A daily budget stop continues with a plain resume: the run's own limits were not reached.
  let RunStatus::Paused(PauseReason::LimitReached(limit @ (RunLimit::Tokens | RunLimit::Time))) = run.run.status()
  else {
    return None;
  };
  let props = RaiseLimitProps::of(run, *limit, limits?);
  Some(ctx.mount_keyed::<RaiseLimitPanel>(
    &format!("raise-limit-{}-{}", run.id(), run.run.last_sequence().get()),
    props,
  ))
}

/// The latest agent launch (`AY4k5`, `P3P5R`): started, finished, stopped (by a pause, a cancel,
/// a limit or the runner) or failed.
fn latest(ctx: &Ctx, run: &RunView) -> Option<Element> {
  let invocation = run.invocations.last()?;
  let step = run.step_of(invocation.execution)?;
  let (key, at) = match (invocation.ended_at, &invocation.outcome) {
    (None, _) => ("runs.summary.latestStarted", invocation.started_at),
    (Some(ended), Some(InvocationOutcome::Completed)) => ("runs.summary.latestEnded", ended),
    (Some(ended), Some(InvocationOutcome::Terminated)) => ("runs.summary.latestStopped", ended),
    (Some(ended), _) => ("runs.summary.latestFailed", ended),
  };
  let line = ctx.t_args(
    key,
    [
      ("time", display::clock(at).as_str()),
      ("step", &step.name),
      ("agent", agent_of(step)),
    ],
  );
  Some(
    Column::new()
      .spacing(ExtraSpacing::Form.role())
      .child(
        Text::new(&ctx.t("runs.summary.latest"))
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary),
      )
      .child(Text::new(&line).variant(TypographyStyle::Caption).id("task-latest"))
      .into(),
  )
}

/// The run's pinned configuration, its limits and usage (`F5Lsi`).
fn this_run(ctx: &Ctx, run: &RunView) -> Element {
  let version = run.version.to_string();
  let pipeline = ctx.t_args(
    "runs.summary.pipelineValue",
    [("name", &*run.pipeline), ("version", &version)],
  );
  let usage = run.usage();
  let cached = ctx.t_args(
    "runs.cachedValue",
    [("count", compact_tokens(usage.cached_input).as_str())],
  );
  let time = message_text(ctx, &display::duration(run.active_time(Timestamp::now())));
  let time_limit = ctx.t_args(
    "runs.summary.timeLimitValue",
    [("minutes", whole_minutes(run.run.max_duration()).to_string().as_str())],
  );
  let step = run.current_step();
  let model = step.and_then(|step| step.model.as_deref()).unwrap_or_default();
  let permissions = step.map(|step| permissions(ctx, step));
  let checkpoint = run.run.last_checkpoint().map(|checkpoint| {
    ctx.t_args(
      "runs.summary.checkpointValue",
      [
        ("revision", display::short_revision(checkpoint.revision().as_str())),
        ("time", display::clock(checkpoint.created_at()).as_str()),
      ],
    )
  });
  Column::new()
    .id("task-this-run")
    .width(Dimension::Px(PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Step.role())
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("runs.summary.thisRun")).variant(ExtraTypography::CardTitle.role()))
    .child(property(&ctx.t("runs.summary.pipeline"), &pipeline))
    .child(styled_property(
      &ctx.t("runs.summary.branch"),
      run.run.branch().as_str(),
      ValueStyle::Mono,
    ))
    .child(styled_property(
      &ctx.t("runs.summary.base"),
      display::short_revision(run.run.base().as_str()),
      ValueStyle::Mono,
    ))
    .with_children(checkpoint.map(|value| property(&ctx.t("runs.summary.checkpoint"), &value)))
    .child(styled_property(&ctx.t("runs.summary.model"), model, ValueStyle::Mono))
    .child(property(&ctx.t("runs.summary.budget"), &budget_value(ctx, run)))
    .child(property(&ctx.t("runs.cached"), &cached))
    .child(property(&ctx.t("runs.summary.timeLimit"), &time_limit))
    .child(property(&ctx.t("runs.summary.time"), &time))
    .with_children(permissions.map(|value| property(&ctx.t("runs.summary.permissions"), &value)))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("runs.summary.snapshotNote")).variant(ExtraTypography::Detail.role()))
    .into()
}

/// Spend reads used / budget in the compact form: "412k / 1.4M tokens", and says when the budget
/// is used up.
pub(super) fn budget_value(ctx: &Ctx, run: &RunView) -> Arc<str> {
  let key = if run.run.token_limit_reached() {
    "tokens.usedOfBudgetReached"
  } else {
    "tokens.usedOfBudget"
  };
  ctx.t_args(
    key,
    [
      ("used", compact_tokens(run.run.used().get()).as_str()),
      ("budget", compact_tokens(run.run.budget().get()).as_str()),
    ],
  )
}

/// The step's agent and what it may do to the repository, e.g. "Builder · Edit files".
pub(super) fn permissions(ctx: &Ctx, step: &StepView) -> std::sync::Arc<str> {
  let access = match step.access {
    Some(RepositoryAccess::Write) => ctx.t("runs.permissions.write"),
    Some(RepositoryAccess::ReadOnly) | None => ctx.t("runs.permissions.readOnly"),
  };
  ctx.t_args(
    "runs.permissions.value",
    [("agent", agent_of(step)), ("access", &access)],
  )
}
