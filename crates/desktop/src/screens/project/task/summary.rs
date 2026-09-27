//! The Summary tab (`Nlcnu` in 02.10 `RD4pc`): what the run is doing in a notice, for a run
//! stopped at a limit the panel that raises it (02.L `ZUYM0`), the run's progress per step
//! (`qB7mj` in 02.12, `l3hyb` rows), the latest agent launch, and the run's pinned configuration,
//! limits and usage in the "This run" panel (`F5Lsi`). A run whose step used up its retries shows
//! 02.B (`J3JZaT`) instead.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::stream::InvocationOutcome,
  limits::ProjectLimits,
  run::{PauseReason, RunFailure, RunLimit, RunStatus},
  time::Timestamp,
  tokens::compact_tokens,
};

use super::{
  heading::agent_of,
  limit_panel::{RaiseLimitPanel, RaiseLimitProps},
  notice::{Lost, notice},
  permissions::{RunPermissionsProps, RunPermissionsRow},
  progress::progress_list,
  retry_limit::retry_limit,
};
use crate::{
  budget::whole_minutes,
  display,
  tasks::detail::{RunView, TaskDetail},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    message_text,
    property::{ValueStyle, property, styled_property},
  },
};

/// "This run" panel width (`F5Lsi`).
const PANEL_WIDTH: f32 = 336.0;

/// `lost`: the run is recorded as running but has no runner, and why.
pub(super) fn summary(
  ctx: &mut Ctx,
  detail: &TaskDetail,
  limits: Option<ProjectLimits>,
  lost: Option<Lost>,
  repo_root: &Path,
) -> Element {
  if let Some(run) = &detail.run
    && let RunStatus::Failed(RunFailure::RetriesExhausted(step)) = run.run.status()
  {
    return retry_limit(ctx, run, step);
  }
  let raise = detail.run.as_ref().and_then(|run| raise_panel(ctx, run, limits));
  let raising = raise.is_some();
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Section)
    .padding_left(SpacingSize::Section)
    .padding_right(ExtraSpacing::BesideGutter.role())
    .spacing(ExtraSpacing::Form.role())
    .child(notice(ctx, detail, lost))
    .with_children(raise)
    .with_children(detail.run.as_ref().map(|run| progress_list(ctx, run, lost.is_some())));
  let latest = detail.run.as_ref().and_then(|run| latest(ctx, run));
  Row::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(progress_column(content, latest, raising))
    .with_children(detail.run.as_ref().map(|run| this_run(ctx, run, repo_root)))
    .into()
}

/// The notice, the raise panel and the progress rows (`content`) in a viewport with the scroll
/// gutter (`ZVdRa`, thumb `PigH4`), so nothing is cut when they are taller than the window.
/// Latest activity sits at the bottom when there is room. With the raise panel the column is
/// taller than a 960px window (02.L2), so Latest activity scrolls with it, below the fold;
/// otherwise it stays below the viewport. lurq cannot make scrolled content at least as tall as
/// its viewport, which both at once would need.
fn progress_column(content: Column, latest: Option<Element>, raising: bool) -> Element {
  let scrolled = |content: Column| {
    ScrollVertical::new(content)
      .id("task-progress-scroll")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .scrollbar(gutter_scrollbar())
  };
  let column = Column::new()
    .id("task-progress")
    .flex(1.0)
    .height(Dimension::Pct(100.0));
  if raising {
    return column
      .child(scrolled(
        content.with_children(latest).padding_bottom(SpacingSize::Section),
      ))
      .into();
  }
  let latest = latest.map(|latest| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Form.role())
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(SpacingSize::Section)
      .child(latest)
  });
  column.child(scrolled(content)).with_children(latest).into()
}

/// Raise the limit to continue (`ZUYM0`), for a run stopped at a limit.
fn raise_panel(ctx: &mut Ctx, run: &RunView, limits: Option<ProjectLimits>) -> Option<Element> {
  // A daily budget stop continues with a plain resume: the run's own limits were not reached.
  let RunStatus::Paused(PauseReason::LimitReached(limit)) = run.run.status() else {
    return None;
  };
  if *limit == RunLimit::DailyBudget {
    return None;
  }
  let props = RaiseLimitProps::of(run, *limit, limits?)?;
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
fn this_run(ctx: &mut Ctx, run: &RunView, repo_root: &Path) -> Element {
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
  // A tool step's command runs on no model.
  let command_tool = ctx.t("runs.activity.commandTool");
  let model = match step {
    Some(step) if step.tool_step => &*command_tool,
    _ => step.and_then(|step| step.model.as_deref()).unwrap_or_default(),
  };
  let permissions = permissions_row(ctx, run, repo_root, false);
  let checkpoint = run.run.last_checkpoint().map(|checkpoint| {
    ctx.t_args(
      "runs.summary.checkpointValue",
      [
        ("revision", display::short_revision(checkpoint.revision().as_str())),
        ("time", display::clock(checkpoint.created_at()).as_str()),
      ],
    )
  });
  let rows = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
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
    .with_children(permissions);
  this_run_panel(ctx, rows)
}

/// The *Permissions* row of the current step's agent, with its *change ↗* (`C5Ai5o`), beside the
/// value when the column is wide enough (`inline_link`); `None` for a tool step, or once its
/// profile version is gone.
pub(super) fn permissions_row(ctx: &mut Ctx, run: &RunView, repo_root: &Path, inline_link: bool) -> Option<Element> {
  let step = run.current_step()?;
  let props = RunPermissionsProps {
    permissions: step.permissions.clone()?,
    step: step.clone(),
    run: run.id(),
    run_number: run.run.number().get(),
    live: !run.run.status().is_terminal(),
    repo_root: repo_root.to_path_buf(),
    inline_link,
  };
  Some(ctx.mount_keyed::<RunPermissionsRow>(&format!("task-permissions-{}", step.key), props))
}

/// The "This run" panel around its `rows`. They scroll when a permission change's notices make
/// them taller than the window, their bar in a gutter of its own that takes the right padding's
/// inner half, so the values still end 24 from the edge (design doc, "Scroll gutter"); the note
/// stays at the bottom.
fn this_run_panel(ctx: &Ctx, rows: Column) -> Element {
  let note = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_right(ExtraSpacing::Stack.role())
    .child(Text::new(&ctx.t("runs.summary.snapshotNote")).variant(ExtraTypography::Detail.role()));
  Column::new()
    .id("task-this-run")
    .width(Dimension::Px(PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_left(SpacingSize::Lg)
    .padding_right(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Step.role())
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      ScrollVertical::new(rows)
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .child(note)
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
