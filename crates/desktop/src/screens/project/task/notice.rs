//! Where the task stands and what that means (`m3kjHT` in 02.10; `JFQSD` paused in 02.A;
//! `EzWLg` stopped at limit in 02.L).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  run::{PauseReason, RunLimit, RunStatus},
  task::TaskStatus,
  time::Timestamp,
  tokens::compact_tokens,
};

use crate::{
  budget::whole_minutes,
  display,
  tasks::detail::{RunView, TaskDetail},
  ui::{icon::Icon, notice::Notice},
};

pub(super) fn notice(ctx: &Ctx, detail: &TaskDetail) -> Element {
  let (glyph, title, text) = match &detail.run {
    None if detail.task.status() == TaskStatus::Draft => (
      Icon::Pencil,
      ctx.t("runs.summary.notice.draftTitle"),
      ctx.t("runs.summary.notice.draftDetail"),
    ),
    None => (
      Icon::Clock3,
      ctx.t("runs.summary.notice.queuedTitle"),
      ctx.t("runs.summary.notice.queuedDetail"),
    ),
    Some(run) => run_notice(ctx, run),
  };
  Notice::new(glyph, title, text).id("task-notice").into()
}

fn run_notice(ctx: &Ctx, run: &RunView) -> (Icon, Arc<str>, Arc<str>) {
  let branch = run.run.branch().as_str();
  let step = run.current_step().map_or("", |step| &*step.name);
  let args = [("step", step), ("branch", branch)];
  let plain = |glyph, title, text| (glyph, ctx.t_args(title, args), ctx.t_args(text, args));
  match run.run.status() {
    RunStatus::Pending => plain(
      Icon::Clock3,
      "runs.summary.notice.queuedTitle",
      "runs.summary.notice.queuedDetail",
    ),
    RunStatus::Running if run.run.pause_requested() => plain(
      Icon::Loader,
      "runs.summary.notice.pausingTitle",
      "runs.summary.notice.pausingDetail",
    ),
    RunStatus::Running => plain(
      Icon::Loader,
      "runs.summary.notice.runningTitle",
      "runs.summary.notice.runningDetail",
    ),
    RunStatus::Paused(PauseReason::Requested) => paused(ctx, run),
    RunStatus::Paused(PauseReason::LimitReached(limit)) => stopped_at_limit(ctx, run, *limit),
    RunStatus::Interrupted(_) => plain(
      Icon::Pause,
      "runs.summary.notice.interruptedTitle",
      "runs.summary.notice.interruptedDetail",
    ),
    RunStatus::Succeeded => plain(
      Icon::CircleCheck,
      "runs.summary.notice.succeededTitle",
      "runs.summary.notice.succeededDetail",
    ),
    RunStatus::Failed(_) => plain(
      Icon::AlertCircle,
      "runs.summary.notice.failedTitle",
      "runs.summary.notice.failedDetail",
    ),
    RunStatus::Cancelled => plain(
      Icon::Square,
      "runs.summary.notice.cancelledTitle",
      "runs.summary.notice.cancelledDetail",
    ),
  }
}

/// "Paused at checkpoint 4b92ad0" (`JFQSD`).
fn paused(ctx: &Ctx, run: &RunView) -> (Icon, Arc<str>, Arc<str>) {
  let title = match run.run.last_checkpoint() {
    Some(checkpoint) => ctx.t_args(
      "runs.summary.notice.pausedTitle",
      [("checkpoint", display::short_revision(checkpoint.revision().as_str()))],
    ),
    None => ctx.t("runs.summary.notice.pausedNoCheckpointTitle"),
  };
  (Icon::Pause, title, ctx.t("runs.summary.notice.pausedDetail"))
}

/// "Stopped at the token limit · checkpoint 7c1e9f2" (`EzWLg`): what was used, and whether the
/// work at the limit was committed.
fn stopped_at_limit(ctx: &Ctx, run: &RunView, limit: RunLimit) -> (Icon, Arc<str>, Arc<str>) {
  let checkpoint = run
    .run
    .last_checkpoint()
    .filter(|_| run.saved_at_pause)
    .map(|checkpoint| display::short_revision(checkpoint.revision().as_str()).to_owned());
  let (title, titled) = match limit {
    RunLimit::Tokens => ("runs.limit.tokenTitle", "runs.limit.tokenTitleCheckpoint"),
    RunLimit::Time => ("runs.limit.timeTitle", "runs.limit.timeTitleCheckpoint"),
  };
  let title = match &checkpoint {
    Some(checkpoint) => ctx.t_args(titled, [("checkpoint", checkpoint.as_str())]),
    None => ctx.t(title),
  };
  let number = run.run.number().get().to_string();
  let agent = run
    .stopped_step()
    .and_then(|step| step.agent.clone())
    .unwrap_or_else(|| ctx.t("runs.limit.theAgent"));
  let (used, of) = match limit {
    RunLimit::Tokens => (
      compact_tokens(run.run.used().get()),
      compact_tokens(run.run.budget().get()),
    ),
    RunLimit::Time => (
      whole_minutes(run.active_time(Timestamp::now())).to_string(),
      whole_minutes(run.run.max_duration()).to_string(),
    ),
  };
  let key = match (limit, checkpoint.is_some()) {
    (RunLimit::Tokens, true) => "runs.limit.tokenDetail",
    (RunLimit::Tokens, false) => "runs.limit.tokenDetailUnsaved",
    (RunLimit::Time, true) => "runs.limit.timeDetail",
    (RunLimit::Time, false) => "runs.limit.timeDetailUnsaved",
  };
  let text = ctx.t_args(
    key,
    [
      ("run", number.as_str()),
      ("used", used.as_str()),
      ("limit", of.as_str()),
      ("agent", &agent),
    ],
  );
  (Icon::OctagonPause, title, text)
}
