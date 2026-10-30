//! What a run of the merge queue says, shared by the Merge queue screen and the task page: the base branch it delivers
//! into, its place in line, how its pipeline delivers, why it left the queue and how long a phase has run.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  merge_queue::Left,
  resource::{DeliverMode, MergeMethod},
  time::Timestamp,
};
use orchester_workspace::{display::short_revision, merge_queue::QueueRow};

/// The base branch `row` delivers into, as its words name it.
pub(crate) fn base(row: &QueueRow) -> &str {
  row.base_branch.as_ref().map_or("", |branch| branch.as_str())
}

/// `1st in line`, `2nd in line`, …
pub(crate) fn place(ctx: &Ctx, place: usize) -> Arc<str> {
  match place {
    1 => ctx.t("delivery.queue.place.first"),
    2 => ctx.t("delivery.queue.place.second"),
    3 => ctx.t("delivery.queue.place.third"),
    other => ctx.t_args("delivery.queue.place.other", [("place", other.to_string())]),
  }
}

pub(crate) fn mode(ctx: &Ctx, mode: DeliverMode) -> Arc<str> {
  match mode {
    DeliverMode::ApproveToMerge => ctx.t("delivery.queue.mode.approve"),
    DeliverMode::AutoMerge => ctx.t("delivery.queue.mode.auto"),
  }
}

pub(crate) fn method(ctx: &Ctx, method: MergeMethod) -> Arc<str> {
  match method {
    MergeMethod::MergeCommit => ctx.t("delivery.queue.method.mergeCommit"),
    MergeMethod::Squash => ctx.t("delivery.queue.method.squash"),
  }
}

/// Why a run left the queue.
pub(crate) fn left_reason(ctx: &Ctx, left: &Left) -> Arc<str> {
  match left {
    Left::Conflicts { files } => ctx.t_args(
      "delivery.queue.detail.conflicts",
      [("count", files.len().to_string()), ("files", files.join(", "))],
    ),
    Left::CheckFailed { tool, detail } if detail.trim().is_empty() => {
      ctx.t_args("delivery.queue.detail.checkFailed", [("tool", tool.as_str())])
    }
    Left::CheckFailed { tool, detail } => ctx.t_args(
      "delivery.queue.detail.checkFailedSaid",
      [("tool", tool.clone()), ("detail", last_line(detail))],
    ),
    Left::KeepsMoving { base, heads } => ctx.t_args(
      "delivery.queue.detail.keepsMoving",
      [
        ("base", base.clone()),
        ("count", heads.len().to_string()),
        (
          "heads",
          heads
            .iter()
            .map(|head| short_revision(head.as_str()).to_owned())
            .collect::<Vec<_>>()
            .join(", "),
        ),
      ],
    ),
    Left::Stopped { detail } => ctx.t_args("delivery.queue.detail.stopped", [("detail", last_line(detail))]),
    Left::LaterRun => ctx.t("delivery.queue.detail.laterRun"),
  }
}

/// The last line of what a program said, as a row has room for it.
fn last_line(text: &str) -> String {
  text
    .lines()
    .rev()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .unwrap_or_default()
    .to_owned()
}

/// How long a phase has run, as a running check says it: `48 s`, `1 min 12 s`, `1 h 4 min`.
pub(crate) fn elapsed(ctx: &Ctx, since: Timestamp, now: Timestamp) -> Arc<str> {
  let seconds = (now.unix_millis() - since.unix_millis()).max(0) / 1000;
  let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
  if hours > 0 {
    ctx.t_args(
      "delivery.queue.elapsed.hours",
      [("hours", hours.to_string()), ("minutes", minutes.to_string())],
    )
  } else if minutes > 0 {
    ctx.t_args(
      "delivery.queue.elapsed.minutes",
      [("minutes", minutes.to_string()), ("seconds", seconds.to_string())],
    )
  } else {
    ctx.t_args("delivery.queue.elapsed.seconds", [("seconds", seconds.to_string())])
  }
}
