//! The Summary tab (`Nlcnu` in 02.10 `RD4pc`): what the run is doing in a notice, the pipeline's
//! steps with their state (`mBkeJ` rows), the latest agent launch, and the run's pinned
//! configuration and usage in the "This run" panel (`F5Lsi`).

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
  resource::RepositoryAccess,
  run::{RunStatus, compact_tokens},
  task::TaskStatus,
  time::Timestamp,
};

use super::heading::{agent_of, step_state};
use crate::{
  display,
  tasks::detail::{RunView, StepView, TaskDetail},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
    property::{ValueStyle, property, styled_property},
  },
};

/// "This run" panel width, step row height and its glyph tile (`F5Lsi`, `NBmWz`, `tWFDE`).
const PANEL_WIDTH: f32 = 336.0;
const STEP_ROW_HEIGHT: f32 = 64.0;
const GLYPH_SIZE: f32 = 28.0;

pub(super) fn summary(ctx: &Ctx, detail: &TaskDetail) -> Element {
  let mut progress = Column::new()
    .id("task-progress")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Form.role())
    .child(notice(ctx, detail))
    .child(Text::new(&ctx.t("runs.summary.execution")).variant(ExtraTypography::SectionHeading.role()));
  if let Some(run) = &detail.run {
    progress = progress
      .with_children(run.steps.iter().map(|step| step_row(ctx, step)))
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

/// Where the task stands and what that means (`m3kjHT`).
fn notice(ctx: &Ctx, detail: &TaskDetail) -> Element {
  let branch = detail.run.as_ref().map_or("", |run| run.run.branch().as_str());
  let step = detail
    .run
    .as_ref()
    .and_then(RunView::current_step)
    .map_or("", |step| &*step.name);
  let (glyph, title, text) = match detail.run.as_ref().map(|run| run.run.status()) {
    None if detail.task.status() == TaskStatus::Draft => (
      Icon::Pencil,
      "runs.summary.notice.draftTitle",
      "runs.summary.notice.draftDetail",
    ),
    None | Some(RunStatus::Pending) => (
      Icon::Clock3,
      "runs.summary.notice.queuedTitle",
      "runs.summary.notice.queuedDetail",
    ),
    Some(RunStatus::Running) => (
      Icon::Loader,
      "runs.summary.notice.runningTitle",
      "runs.summary.notice.runningDetail",
    ),
    Some(RunStatus::Paused | RunStatus::Interrupted(_)) => (
      Icon::Pause,
      "runs.summary.notice.interruptedTitle",
      "runs.summary.notice.interruptedDetail",
    ),
    Some(RunStatus::Succeeded) => (
      Icon::CircleCheck,
      "runs.summary.notice.succeededTitle",
      "runs.summary.notice.succeededDetail",
    ),
    Some(RunStatus::Failed(_)) => (
      Icon::AlertCircle,
      "runs.summary.notice.failedTitle",
      "runs.summary.notice.failedDetail",
    ),
    Some(RunStatus::Cancelled) => (
      Icon::Square,
      "runs.summary.notice.cancelledTitle",
      "runs.summary.notice.cancelledDetail",
    ),
  };
  let args = [("step", step), ("branch", branch)];
  Notice::new(glyph, ctx.t_args(title, args), ctx.t_args(text, args))
    .id("task-notice")
    .into()
}

/// One step and its state (`mBkeJ` as in `NBmWz`).
fn step_row(ctx: &Ctx, step: &StepView) -> Element {
  let (glyph, state) = step_state(step.status);
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

/// The latest agent launch (`AY4k5`, `P3P5R`).
fn latest(ctx: &Ctx, run: &RunView) -> Option<Element> {
  let invocation = run.invocations.last()?;
  let step = run.step_of(invocation.execution)?;
  let (key, at) = match invocation.ended_at {
    Some(ended) => ("runs.summary.latestEnded", ended),
    None => ("runs.summary.latestStarted", invocation.started_at),
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
      .spacing(SpacingSize::Sm)
      .child(
        Text::new(&ctx.t("runs.summary.latest"))
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary),
      )
      .child(Text::new(&line).variant(TypographyStyle::Caption).id("task-latest"))
      .into(),
  )
}

/// The run's pinned configuration, its usage and its time (`F5Lsi`).
fn this_run(ctx: &Ctx, run: &RunView) -> Element {
  let version = run.version.to_string();
  let pipeline = ctx.t_args(
    "runs.summary.pipelineValue",
    [("name", &*run.pipeline), ("version", &version)],
  );
  let usage = run.usage();
  let tokens = ctx.t_args("runs.tokens", [("count", compact_tokens(usage.counted()).as_str())]);
  let cached = ctx.t_args(
    "runs.cachedValue",
    [("count", compact_tokens(usage.cached_input).as_str())],
  );
  let time = message_text(
    ctx,
    &display::duration(std::time::Duration::from_millis(
      u64::try_from(run.active_millis(Timestamp::now())).unwrap_or_default(),
    )),
  );
  let step = run.current_step();
  let model = step.and_then(|step| step.model.as_deref()).unwrap_or_default();
  let permissions = step.map(|step| permissions(ctx, step));
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
    .child(styled_property(&ctx.t("runs.summary.model"), model, ValueStyle::Mono))
    .child(property(&ctx.t("runs.summary.tokens"), &tokens))
    .child(property(&ctx.t("runs.cached"), &cached))
    .child(property(&ctx.t("runs.summary.time"), &time))
    .with_children(permissions.map(|value| property(&ctx.t("runs.summary.permissions"), &value)))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("runs.summary.snapshotNote")).variant(ExtraTypography::Detail.role()))
    .into()
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
