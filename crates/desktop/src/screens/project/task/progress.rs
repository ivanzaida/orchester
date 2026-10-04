//! The run's progress (`AVgu9` in 02.S `JOG1i`): a check row (`l3hyb`) per step, including
//! parallel steps that have not started, with where it stands, its attempt, tokens and active
//! agent time ("Working · attempt 2 · 12k tokens · 3 min").

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{run::StepFailure, time::Timestamp, tokens::compact_tokens};

use super::approval_card::waiting_on;
use crate::{
  display,
  tasks::{
    approvals::ShownApproval,
    detail::RunView,
    progress::{ProgressRow, ProgressState, progress},
  },
  ui::{check_row::check_row, icon::Icon, message_text},
};

/// Every step of `run`. `lost`: the run is recorded as running but has no runner. A step whose agent
/// waits on one of `waiting`, the requests of the run, says what it waits on and since when (`G0rzJy`),
/// whatever its recorded state: a tool request does not change the step's state, its agent is at work.
pub(super) fn progress_list(ctx: &Ctx, run: &RunView, lost: bool, waiting: &[ShownApproval]) -> Element {
  let rows = progress(run, lost, Timestamp::now());
  Column::new()
    .id("task-progress-steps")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(
      rows
        .iter()
        .map(|row| progress_row(ctx, row, waiting_on(ctx, waiting, &row.id))),
    )
    .into()
}

/// `rows` as check rows, one below the other, `gap` apart: 8 in the progress list (`qB7mj`), 22 in
/// the retry limit's evidence (`yYFkk`).
pub(super) fn rows(ctx: &Ctx, rows: &[ProgressRow], gap: SpacingSize) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(gap)
    .with_children(rows.iter().map(|row| progress_row(ctx, row, None)))
}

/// A step's check row; `waiting_on` replaces its detail while it waits on a request.
fn progress_row(ctx: &Ctx, row: &ProgressRow, waiting_on: Option<Arc<str>>) -> Element {
  let title = title(ctx, row);
  let (glyph, state) = state(ctx, row.state);
  let (glyph, result) = match waiting_on {
    Some(waiting_on) => (Icon::Hand, waiting_on),
    None => (glyph, detail(ctx, row, state)),
  };
  check_row(glyph, &title, &result)
    .describe("step", row.id.to_string())
    .describe("state", format!("{:?}", row.state))
    .into()
}

/// Where a step stands, with its attempt, tokens and active agent time when it ran.
fn detail(ctx: &Ctx, row: &ProgressRow, state: Arc<str>) -> Arc<str> {
  match row.usage {
    Some(usage) => {
      let attempt = usage.attempt.to_string();
      let duration = message_text(ctx, &display::duration(usage.active));
      match usage.tokens {
        Some(tokens) => ctx.t_args(
          "runs.progress.detail",
          [
            ("state", &*state),
            ("attempt", attempt.as_str()),
            ("tokens", compact_tokens(tokens).as_str()),
            ("duration", &*duration),
          ],
        ),
        // A tool step's command uses no tokens.
        None => ctx.t_args(
          "runs.progress.commandDetail",
          [
            ("state", &*state),
            ("attempt", attempt.as_str()),
            ("duration", &*duration),
          ],
        ),
      }
    }
    None => state,
  }
}

/// "Implement · Builder"; a step with no available agent shows its name alone.
fn title(ctx: &Ctx, row: &ProgressRow) -> Arc<str> {
  match &row.agent {
    Some(agent) => ctx.t_args("runs.progress.stepAgent", [("step", &*row.step), ("agent", &**agent)]),
    None => row.step.clone(),
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
    ProgressState::CantContinue => (Icon::CirclePause, "runs.progress.state.cantContinue"),
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
    Some(StepFailure::CommandFailed) => "runs.progress.failure.commandFailed",
  }
}
