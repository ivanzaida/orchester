//! A run row's worktree line (`w3zBo` in 63.01, 63.06 `VBR27`): what the run's worktrees hold, that
//! they were removed and when, that they are in use, that the run never had one, or that nothing
//! can be said while the run cannot be read.

use std::sync::Arc;

use chrono::Datelike;
use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  integration::Leftovers,
  run::{PauseReason, Run, RunStatus},
  time::Timestamp,
};

use crate::{
  display::{self, MONTH_KEYS},
  messages::Message,
  tasks::history::RunRecord,
  ui::{icon::Icon, message_text},
};

/// The line's glyph, text and color.
pub(super) struct Line {
  pub(super) glyph: Icon,
  pub(super) text: Arc<str>,
  pub(super) color: PaletteColor,
}

impl Line {
  fn muted(glyph: Icon, text: Arc<str>) -> Self {
    Self {
      glyph,
      text,
      color: PaletteColor::TextSecondary,
    }
  }
}

/// The line of `record`, whose run left what `left` says; `None` until that was read.
pub(super) fn line(ctx: &Ctx, record: &RunRecord, left: Option<&Result<Leftovers, Message>>) -> Option<Line> {
  let Some(run) = &record.run else {
    let text = match record.number {
      Some(number) => ctx.t_args(
        "runs.history.worktree.unreadable",
        [("run", number.get().to_string().as_str())],
      ),
      None => ctx.t("runs.history.worktree.unreadableUnnumbered"),
    };
    return Some(Line::muted(Icon::Lock, text));
  };
  if run.current_attempt().is_none() {
    return Some(Line::muted(Icon::Minus, ctx.t("runs.history.worktree.none")));
  }
  if run.status().has_live_work() {
    let key = match run.status() {
      RunStatus::Paused(PauseReason::Requested) => "runs.history.worktree.keptForResume",
      RunStatus::Paused(PauseReason::LimitReached(_)) => "runs.history.worktree.keptToContinue",
      _ => "runs.history.worktree.inUse",
    };
    return Some(Line::muted(Icon::Folder, ctx.t(key)));
  }
  match left? {
    Err(error) => Some(Line::muted(Icon::AlertCircle, message_text(ctx, error))),
    Ok(left) if left.worktrees.is_empty() => Some(removed(ctx, run, record.removed_at, left)),
    Ok(left) => Some(present(ctx, run, left)),
  }
}

/// "Worktree removed Oct 3, 16:40 · orc/switcher-shortcuts-3 kept"; without the time when
/// Orchester did not record it (the folder was deleted by hand, or before ORC-63).
fn removed(ctx: &Ctx, run: &Run, at: Option<Timestamp>, left: &Leftovers) -> Line {
  let branch = run.branch().as_str();
  let kept = left.branches.iter().any(|left| left.name == *run.branch());
  let when = at.and_then(|at| day_and_clock(ctx, at));
  let text = match (when, kept) {
    (Some(time), true) => ctx.t_args(
      "runs.history.worktree.removedAt.kept",
      [("time", &*time), ("branch", branch)],
    ),
    (Some(time), false) => ctx.t_args(
      "runs.history.worktree.removedAt.deleted",
      [("time", &*time), ("branch", branch)],
    ),
    (None, true) => ctx.t_args("runs.history.worktree.removed.kept", [("branch", branch)]),
    (None, false) => ctx.t_args("runs.history.worktree.removed.deleted", [("branch", branch)]),
  };
  Line::muted(Icon::FolderX, text)
}

/// "Worktree · 2 files not committed · orc/switcher-shortcuts", in `text` when files would be lost.
fn present(ctx: &Ctx, run: &Run, left: &Leftovers) -> Line {
  let branch = run.branch().as_str();
  let worktrees = left.worktrees.len().to_string();
  let files = left.uncommitted();
  let count = files.to_string();
  let args = [
    ("branch", branch),
    ("worktrees", worktrees.as_str()),
    ("count", count.as_str()),
  ];
  let several = left.worktrees.len() > 1;
  let key = match (several, files) {
    (false, _) if left.worktrees.iter().all(|worktree| worktree.missing) => "runs.history.worktree.missing",
    (false, 0) => "runs.history.worktree.clean",
    (false, 1) => "runs.history.worktree.uncommitted.one",
    (false, _) => "runs.history.worktree.uncommitted.other",
    (true, 0) => "runs.history.worktree.several.clean",
    (true, 1) => "runs.history.worktree.several.uncommitted.one",
    (true, _) => "runs.history.worktree.several.uncommitted.other",
  };
  let text = ctx.t_args(key, args);
  if files == 0 {
    return Line::muted(Icon::Folder, text);
  }
  Line {
    glyph: Icon::FileDiff,
    text,
    color: PaletteColor::TextPrimary,
  }
}

/// Local date and time of `at`, e.g. "Sep 26 · 11:04"; `None` for an instant chrono cannot place,
/// which no stored change has.
pub(super) fn day_and_clock(ctx: &Ctx, at: Timestamp) -> Option<Arc<str>> {
  let local = chrono::DateTime::from_timestamp_millis(at.unix_millis())?.with_timezone(&chrono::Local);
  let month = ctx.t(MONTH_KEYS[local.month0() as usize]);
  let day = local.day().to_string();
  let time = display::clock(at);
  Some(ctx.t_args(
    "time.dayAndClock",
    [("month", &*month), ("day", day.as_str()), ("time", time.as_str())],
  ))
}
