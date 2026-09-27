//! The task page heading (`DAsh8`): the task's title and where its run stands, e.g. "Run 128 ·
//! Attempt 1 · Implement · Builder working", and the words and icons for step states. A run ready
//! for review shows its status label and when it finished instead (`ruBkY` in 02.13 `vpgrJ`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::{PauseReason, RunFailure, RunLimit, RunStatus, StepStatus, TaskPhase};

use super::super::page;
use crate::{
  display,
  tasks::detail::{RunView, StepView, TaskDetail},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{icon::Icon, status_label::status_label},
};

/// `lost`: the run is recorded as running but no runner is connected, so it is interrupted.
pub(super) fn heading(ctx: &Ctx, detail: &TaskDetail, lost: bool) -> Element {
  let title = detail.task.details().title.as_str();
  match &detail.run {
    Some(run) if *run.run.status() == RunStatus::Succeeded => ready_heading(ctx, title, run, detail.merge.is_some()),
    _ => page::heading(title, &description(ctx, detail, lost)),
  }
}

/// The title over "Ready for review" and "Run #128 · Feature delivery · Finished at 14:32" (`ruBkY`);
/// "Merged" once the run's work was merged (not drawn yet).
fn ready_heading(ctx: &Ctx, title: &str, run: &RunView, merged: bool) -> Element {
  let (glyph, status) = if merged {
    (Icon::GitMerge, ctx.t("runs.merge.status"))
  } else {
    (Icon::GitPullRequest, ctx.t("runs.ready.status"))
  };
  let number = run.run.number().get().to_string();
  let finished = run.finished_at.map(display::clock).unwrap_or_default();
  let meta = ctx.t_args(
    "runs.ready.meta",
    [
      ("run", number.as_str()),
      ("pipeline", &*run.pipeline),
      ("time", finished.as_str()),
    ],
  );
  Column::new()
    .id("task-heading")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Rail.role())
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(ExtraSpacing::HeadingBottom.role())
    .spacing(ExtraSpacing::Group.role())
    .child(Text::new(title).variant(ExtraTypography::PageTitle.role()))
    .child(
      Row::new()
        .spacing(ExtraSpacing::Group.role())
        .align_items(Alignment::Center)
        .child(status_label(glyph, &status))
        .child(
          Text::new(&meta)
            .id("task-ready-meta")
            .variant(TypographyStyle::Caption)
            .nowrap(),
        ),
    )
    .into()
}

fn description(ctx: &Ctx, detail: &TaskDetail, lost: bool) -> Arc<str> {
  let Some(run) = &detail.run else {
    return match detail.phase() {
      TaskPhase::Draft => ctx.t("runs.page.draft"),
      _ => ctx.t("runs.page.queued"),
    };
  };
  let number = run.run.number().get().to_string();
  let attempt = run
    .run
    .current_attempt()
    .map_or_else(String::new, |attempt| attempt.number().to_string());
  let step = run.current_step();
  let working: Vec<&str> = run.working_steps().map(|step| &*step.name).collect();
  let parallel = working.join(&ctx.t("runs.progress.together"));
  let step_name = step.map_or("", |step| &step.name);
  let agent = step.and_then(|step| step.agent.as_deref()).unwrap_or_default();
  let time = run.paused_at.map(display::clock).unwrap_or_default();
  let version = run.version.to_string();
  let args = [
    ("run", number.as_str()),
    ("attempt", attempt.as_str()),
    ("step", step_name),
    ("agent", agent),
    ("time", time.as_str()),
    ("steps", parallel.as_str()),
    ("pipeline", &run.pipeline),
    ("version", version.as_str()),
  ];
  let awaiting_plan = run.run.waits_for_a_person() && !run.run.awaiting_plan().is_empty();
  let key = match (run.run.status(), step) {
    (RunStatus::Running, _) if lost => "runs.page.describe.interrupted",
    (RunStatus::Running, _) if awaiting_plan => "runs.page.describe.awaitingPlan",
    (RunStatus::Pending, _) => "runs.page.describe.pending",
    (RunStatus::Running, _) if run.run.pause_requested() => "runs.page.describe.pausing",
    (RunStatus::Running, Some(_)) if working.len() > 1 => "runs.page.describe.workingParallel",
    (RunStatus::Running, Some(_)) => "runs.page.describe.working",
    (RunStatus::Running, None) => "runs.page.describe.starting",
    (RunStatus::Paused(PauseReason::Requested), _) => "runs.page.describe.paused",
    (RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens)), _) => "runs.page.describe.tokenLimit",
    (RunStatus::Paused(PauseReason::LimitReached(RunLimit::Time)), _) => "runs.page.describe.timeLimit",
    (RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget)), _) => "runs.page.describe.dailyBudget",
    (RunStatus::Paused(PauseReason::LimitReached(RunLimit::StepBudget { .. })), _) => "runs.page.describe.stepBudget",
    (RunStatus::Interrupted(_), _) => "runs.page.describe.interrupted",
    (RunStatus::Succeeded, _) => "runs.page.describe.succeeded",
    (RunStatus::Failed(RunFailure::RetriesExhausted(_)), _) => "runs.page.describe.retriesExhausted",
    (RunStatus::Failed(_), _) => "runs.page.describe.failed",
    (RunStatus::Cancelled, _) => "runs.page.describe.cancelled",
  };
  ctx.t_args(key, args)
}

/// Icon and word of a step's state (`DeizF`); a step that has not started waits.
pub(super) fn step_state(status: Option<StepStatus>) -> (Icon, &'static str) {
  match status {
    None | Some(StepStatus::Pending) => (Icon::Clock3, "runs.step.pending"),
    Some(StepStatus::Running) => (Icon::Loader, "runs.step.running"),
    Some(StepStatus::Waiting(_)) => (Icon::Clock3, "runs.step.waiting"),
    Some(StepStatus::Passed) => (Icon::Check, "runs.step.passed"),
    Some(StepStatus::Failed) => (Icon::AlertCircle, "runs.step.failed"),
    Some(StepStatus::Skipped) => (Icon::CircleMinus, "runs.step.skipped"),
  }
}

/// "Builder" or, for a step without an agent, nothing.
pub(super) fn agent_of(step: &StepView) -> &str {
  step.agent.as_deref().unwrap_or_default()
}
