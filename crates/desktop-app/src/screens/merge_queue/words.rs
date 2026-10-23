//! What a run of the merge queue says (`Queue row` `kcUhy`): its state's glyph, word and color, the line under the
//! word, the run's meta line and the detail under it. Shared by the Merge queue screen and the task page.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  integration::MergeKind,
  merge_queue::{Left, QueueEvent, QueueState},
  resource::{DeliverMode, MergeMethod},
  time::Timestamp,
};
use orchester_workspace::{
  display::{clock, short_revision},
  merge_queue::QueueRow,
};

use crate::ui::icon::Icon;

/// How a row's state shows: its glyph, its word and their color.
pub(crate) struct StateLook {
  pub icon: Icon,
  pub icon_color: PaletteColor,
  pub label: Arc<str>,
  pub label_color: PaletteColor,
}

/// The base branch `row` delivers into, as its words name it.
pub(crate) fn base(row: &QueueRow) -> &str {
  row.base_branch.as_ref().map_or("", |branch| branch.as_str())
}

pub(crate) fn state(ctx: &Ctx, row: &QueueRow) -> StateLook {
  let look = |icon, label: Arc<str>| StateLook {
    icon,
    icon_color: PaletteColor::TextSecondary,
    label,
    label_color: PaletteColor::TextPrimary,
  };
  let tinted = |icon, label: Arc<str>, color: PaletteColor| StateLook {
    icon,
    icon_color: color.clone(),
    label,
    label_color: color,
  };
  match &row.entry.state {
    QueueState::Queued | QueueState::Removed => look(Icon::Clock3, ctx.t("delivery.queue.state.queued")),
    QueueState::Syncing => look(
      Icon::LoaderCircle,
      ctx.t_args("delivery.queue.state.syncing", [("base", base(row))]),
    ),
    QueueState::Rechecking { .. } => look(Icon::LoaderCircle, ctx.t("delivery.queue.state.rechecking")),
    QueueState::Integrating { .. } => look(Icon::LoaderCircle, ctx.t("delivery.queue.state.integrating")),
    QueueState::WaitingForApproval => tinted(
      Icon::Hand,
      ctx.t("delivery.queue.state.waitingForApproval"),
      PaletteColor::Warning,
    ),
    QueueState::Blocked(_) => tinted(
      Icon::CirclePause,
      ctx.t("delivery.queue.state.blocked"),
      PaletteColor::Warning,
    ),
    QueueState::Left(left) if left.failed() => tinted(
      Icon::CircleX,
      ctx.t("delivery.queue.state.failed"),
      PaletteColor::Danger,
    ),
    QueueState::Left(_) => tinted(
      Icon::Hand,
      ctx.t("delivery.queue.state.needsYou"),
      PaletteColor::Warning,
    ),
    QueueState::Merged(_) => look(Icon::GitMerge, ctx.t("delivery.queue.state.merged")),
  }
}

/// The line under the state's word: a waiting run's place, how many re-syncs a run is into, since when it is in its
/// state, or when it was merged or left.
pub(crate) fn sub(ctx: &Ctx, row: &QueueRow) -> Arc<str> {
  let entry = &row.entry;
  match &entry.state {
    QueueState::Queued => place(ctx, row.place.unwrap_or(1)),
    QueueState::Syncing if entry.resyncs > 0 => ctx.t_args(
      "delivery.queue.again",
      [
        ("count", entry.resyncs.to_string()),
        ("max", orchester_core::merge_queue::MAX_RESYNCS.to_string()),
      ],
    ),
    QueueState::Merged(_) | QueueState::Left(_) | QueueState::Removed => clock(entry.since).into(),
    _ => ctx.t_args("delivery.queue.since", [("time", clock(entry.since))]),
  }
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

fn merged_method(ctx: &Ctx, kind: MergeKind) -> Arc<str> {
  match kind {
    MergeKind::Squash => ctx.t("delivery.queue.method.squash"),
    MergeKind::MergeCommit | MergeKind::FastForward | MergeKind::AlreadyMerged => {
      ctx.t("delivery.queue.method.mergeCommit")
    }
  }
}

/// The pipeline as a row names it: `Feature delivery v3`.
pub(crate) fn pipeline(ctx: &Ctx, row: &QueueRow) -> Arc<str> {
  ctx.t_args(
    "delivery.queue.empty.pipeline",
    [
      ("name", row.pipeline.0.clone()),
      ("version", row.pipeline.1.to_string()),
    ],
  )
}

/// The run's meta line: its number, pipeline and what fits its state.
pub(crate) fn meta(ctx: &Ctx, row: &QueueRow) -> Arc<str> {
  let run = row.run_number.to_string();
  let pipeline = pipeline(ctx, row);
  let entry = &row.entry;
  let method = method(ctx, row.deliver.method());
  match &entry.state {
    QueueState::Queued => ctx.t_args(
      "delivery.queue.meta.queued",
      [
        ("run", run),
        ("pipeline", pipeline.to_string()),
        ("mode", mode(ctx, row.deliver.mode()).to_string()),
        ("method", method.to_string()),
        ("time", clock(entry.finished_at)),
      ],
    ),
    QueueState::Syncing if entry.resyncs > 0 => moved_meta(ctx, row, (run, pipeline)),
    QueueState::Integrating { base, files, .. } => ctx.t_args(
      "delivery.queue.meta.conflicts",
      [
        ("run", run),
        ("pipeline", pipeline.to_string()),
        ("count", files.len().to_string()),
        ("base", self::base(row).to_owned()),
        ("revision", short_revision(base.as_str()).to_owned()),
      ],
    ),
    QueueState::WaitingForApproval => match &entry.checked {
      Some(checked) => ctx.t_args(
        "delivery.queue.meta.checked",
        [
          ("run", run),
          ("pipeline", pipeline.to_string()),
          ("base", self::base(row).to_owned()),
          ("revision", short_revision(checked.base.as_str()).to_owned()),
        ],
      ),
      None => delivering_meta(ctx, row, (run, pipeline)),
    },
    QueueState::Blocked(_) if entry.approved_at.is_some() => ctx.t_args(
      "delivery.queue.meta.approved",
      [
        ("run", run),
        ("pipeline", pipeline.to_string()),
        ("method", method.to_string()),
      ],
    ),
    QueueState::Merged(delivered) => ctx.t_args(
      "delivery.queue.meta.merged",
      [
        ("run", run),
        ("pipeline", pipeline.to_string()),
        ("revision", short_revision(delivered.revision.as_str()).to_owned()),
        ("method", merged_method(ctx, delivered.kind).to_string()),
      ],
    ),
    QueueState::Left(_) | QueueState::Removed => ctx.t_args(
      "delivery.queue.meta.left",
      [("run", run), ("pipeline", pipeline.to_string())],
    ),
    _ => delivering_meta(ctx, row, (run, pipeline)),
  }
}

fn delivering_meta(ctx: &Ctx, row: &QueueRow, (run, pipeline): (String, Arc<str>)) -> Arc<str> {
  ctx.t_args(
    "delivery.queue.meta.delivering",
    [
      ("run", run),
      ("pipeline", pipeline.to_string()),
      ("mode", mode(ctx, row.deliver.mode()).to_string()),
      ("method", method(ctx, row.deliver.method()).to_string()),
    ],
  )
}

/// A run syncing again: where the base moved to and when.
fn moved_meta(ctx: &Ctx, row: &QueueRow, (run, pipeline): (String, Arc<str>)) -> Arc<str> {
  let moved = row.events.iter().rev().find_map(|recorded| match &recorded.event {
    QueueEvent::CheckSuperseded { now, .. } => Some((now.clone(), recorded.at)),
    _ => None,
  });
  let Some((now, at)) = moved else {
    return delivering_meta(ctx, row, (run, pipeline));
  };
  ctx.t_args(
    "delivery.queue.meta.moved",
    [
      ("run", run),
      ("pipeline", pipeline.to_string()),
      ("base", base(row).to_owned()),
      ("revision", short_revision(now.as_str()).to_owned()),
      ("time", clock(at)),
    ],
  )
}

/// The detail under a row's meta line, when its state has one: why it left, what was cleared after its merge, or
/// that its base moved since its check.
pub(crate) fn detail(ctx: &Ctx, row: &QueueRow) -> Option<Arc<str>> {
  match &row.entry.state {
    QueueState::WaitingForApproval if row.base_moved => {
      Some(ctx.t_args("delivery.queue.detail.baseMoved", [("base", base(row))]))
    }
    QueueState::Merged(delivered) => Some(match (delivered.worktree_removed, delivered.branch_deleted) {
      (true, true) => ctx.t_args("delivery.queue.detail.cleared", [("branch", row.branch.as_str())]),
      (true, false) => ctx.t_args(
        "delivery.queue.detail.clearedKeptBranch",
        [("branch", row.branch.as_str())],
      ),
      (false, _) => ctx.t("delivery.queue.detail.notCleared"),
    }),
    QueueState::Left(left) => Some(left_reason(ctx, left)),
    _ => None,
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
