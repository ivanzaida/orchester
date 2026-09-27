//! The run's progress (`qB7mj` in 02.12 `FjbKY`): a check row (`l3hyb`) per step, or per stage of
//! parallel steps that have not started, with where it stands, its attempt, tokens and active
//! agent time ("Working · attempt 2 · 12k tokens · 3 min").

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{run::StepFailure, time::Timestamp, tokens::compact_tokens};

use crate::{
  display,
  tasks::{
    detail::RunView,
    progress::{ProgressRow, ProgressState, progress},
  },
  ui::{check_row::check_row, icon::Icon, message_text},
};

/// Every step of `run`. `lost`: the run is recorded as running but has no runner.
pub(super) fn progress_list(ctx: &Ctx, run: &RunView, lost: bool) -> Element {
  rows(ctx, &progress(run, lost, Timestamp::now()), SpacingSize::Sm)
    .id("task-progress-steps")
    .into()
}

/// `rows` as check rows, one below the other, `gap` apart: 8 in the progress list (`qB7mj`), 22 in
/// the retry limit's evidence (`yYFkk`).
pub(super) fn rows(ctx: &Ctx, rows: &[ProgressRow], gap: SpacingSize) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(gap)
    .with_children(rows.iter().map(|row| progress_row(ctx, row)))
}

fn progress_row(ctx: &Ctx, row: &ProgressRow) -> Element {
  let title = title(ctx, row);
  let (glyph, state) = state(ctx, row.state);
  let result = match row.usage {
    Some(usage) => {
      let attempt = usage.attempt.to_string();
      let duration = message_text(ctx, &display::duration(usage.active));
      ctx.t_args(
        "runs.progress.detail",
        [
          ("state", &*state),
          ("attempt", attempt.as_str()),
          ("tokens", compact_tokens(usage.tokens).as_str()),
          ("duration", &*duration),
        ],
      )
    }
    None => state,
  };
  check_row(glyph, &title, &result)
    .describe("step", row.id.to_string())
    .describe("state", format!("{:?}", row.state))
    .into()
}

/// "Implement · Builder", or "Checks + Review" for steps shown together.
fn title(ctx: &Ctx, row: &ProgressRow) -> Arc<str> {
  match row.steps.as_slice() {
    [(step, Some(agent))] => ctx.t_args("runs.progress.stepAgent", [("step", &**step), ("agent", &**agent)]),
    [(step, None)] => step.clone(),
    steps => steps
      .iter()
      .map(|(step, _)| &**step)
      .collect::<Vec<_>>()
      .join(&ctx.t("runs.progress.together"))
      .into(),
  }
}

fn state(ctx: &Ctx, state: ProgressState) -> (Icon, Arc<str>) {
  let (glyph, key) = match state {
    ProgressState::Waiting => (Icon::Clock3, "runs.progress.state.waiting"),
    ProgressState::Working => (Icon::Loader, "runs.progress.state.working"),
    ProgressState::NeedsYou => (Icon::MessageSquare, "runs.progress.state.needsYou"),
    ProgressState::Paused => (Icon::Pause, "runs.progress.state.paused"),
    ProgressState::StoppedAtLimit => (Icon::OctagonPause, "runs.progress.state.stoppedAtLimit"),
    ProgressState::Interrupted => (Icon::CirclePause, "runs.progress.state.interrupted"),
    ProgressState::Complete => (Icon::CircleCheck, "runs.progress.state.complete"),
    ProgressState::Failed(failure) => (Icon::CircleX, failure_key(failure)),
    ProgressState::Skipped => (Icon::CircleMinus, "runs.progress.state.skipped"),
    ProgressState::Cancelled => (Icon::Square, "runs.progress.state.cancelled"),
  };
  (glyph, ctx.t(key))
}

/// Why a step failed, in a word or two: "Merge conflict", "Step budget reached".
pub(super) const fn failure_key(failure: Option<StepFailure>) -> &'static str {
  match failure {
    None | Some(StepFailure::AgentFailed) => "runs.progress.state.failed",
    Some(StepFailure::ReportedFailed) => "runs.progress.failure.reported",
    Some(StepFailure::NotStarted) => "runs.progress.failure.notStarted",
    Some(StepFailure::StepBudget) => "runs.progress.failure.stepBudget",
    Some(StepFailure::MergeConflict) => "runs.progress.failure.mergeConflict",
    Some(StepFailure::CheckpointFailed) => "runs.progress.failure.checkpointFailed",
  }
}
