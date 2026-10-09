//! A run row's worktree line (`Worktree` `MJQYp` in `Run row` `Db8k4`): what the run's worktrees hold,
//! that they were removed and when, that they are in use, that the run never had one, or that nothing
//! can be said while the run cannot be read.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  integration::Leftovers,
  run::{PauseReason, Run, RunStatus},
  time::Timestamp,
};
use orchester_workspace::{
  display::{self, MONTH_KEYS},
  i18n::NAMESPACE,
  messages::Message,
  tasks::history::RunRecord,
};

use crate::ui::{icon::Icon, message::message_text};

/// The line's glyph, words and color.
pub struct Line {
  pub glyph: Icon,
  pub text: Arc<str>,
  pub color: PaletteColor,
}

impl Line {
  fn muted(glyph: Icon, text: Arc<str>) -> Self {
    Self {
      glyph,
      text,
      color: PaletteColor::TextMuted,
    }
  }
}

/// The line of `record`, whose run left what `left` says; `None` until that was read.
pub fn line(ctx: &Ctx, record: &RunRecord, left: Option<&Result<Leftovers, Message>>) -> Option<Line> {
  let Some(run) = &record.run else {
    let text = match record.number {
      Some(number) => ctx.t_args("summary.runs.worktree.unreadable", [("run", number.get().to_string())]),
      None => ctx.t("summary.runs.worktree.unreadableUnnumbered"),
    };
    return Some(Line::muted(Icon::Lock, text));
  };
  if run.current_attempt().is_none() {
    return Some(Line::muted(Icon::Minus, ctx.t("summary.runs.worktree.none")));
  }
  let branch = [("branch", run.branch().as_str())];
  if run.status().has_live_work() {
    let key = match run.status() {
      RunStatus::Paused(PauseReason::Requested) => "summary.runs.worktree.keptForResume",
      RunStatus::Running if run.pause_requested() => "summary.runs.worktree.keptForResume",
      RunStatus::Paused(PauseReason::LimitReached(_)) => "summary.runs.worktree.keptToContinue",
      _ => "summary.runs.worktree.inUse",
    };
    return Some(Line::muted(Icon::Folder, ctx.t_args(key, branch)));
  }
  match left? {
    Err(error) => Some(Line::muted(Icon::CircleAlert, message_text(ctx, error))),
    Ok(left) if left.worktrees.is_empty() => Some(removed(ctx, run, record.removed_at, left)),
    Ok(left) => Some(present(ctx, run, left)),
  }
}

/// "Worktree removed Oct 3, 16:40 · kon-12-first kept"; without the time when Orchester did not record
/// it (the folder was deleted by hand).
fn removed(ctx: &Ctx, run: &Run, at: Option<Timestamp>, left: &Leftovers) -> Line {
  let branch = run.branch().as_str();
  let kept = left.branches.iter().any(|left| left.name == *run.branch());
  let text = match (at.map(|at| day_and_clock(ctx, at)), kept) {
    (Some(time), true) => ctx.t_args(
      "summary.runs.worktree.removedAtKept",
      [("time", &*time), ("branch", branch)],
    ),
    (Some(time), false) => ctx.t_args(
      "summary.runs.worktree.removedAtDeleted",
      [("time", &*time), ("branch", branch)],
    ),
    (None, true) => ctx.t_args("summary.runs.worktree.removedKept", [("branch", branch)]),
    (None, false) => ctx.t_args("summary.runs.worktree.removedDeleted", [("branch", branch)]),
  };
  Line::muted(Icon::FolderX, text)
}

/// "Worktree · 2 files not committed · kon-12-retry-v1", in `text_primary` when files would be lost.
fn present(ctx: &Ctx, run: &Run, left: &Leftovers) -> Line {
  let files = left.uncommitted();
  let args = [
    ("branch", run.branch().to_string()),
    ("worktrees", left.worktrees.len().to_string()),
    ("count", files.to_string()),
  ];
  let several = left.worktrees.len() > 1;
  let key = match (several, files) {
    (false, _) if left.worktrees.iter().all(|worktree| worktree.missing) => "summary.runs.worktree.missing",
    (false, 0) => "summary.runs.worktree.clean",
    (false, 1) => "summary.runs.worktree.uncommittedOne",
    (false, _) => "summary.runs.worktree.uncommittedOther",
    (true, 0) => "summary.runs.worktree.severalClean",
    (true, _) => "summary.runs.worktree.severalUncommitted",
  };
  let text = ctx.t_args(key, args);
  if left.loses_files() {
    return Line {
      glyph: Icon::FileDiff,
      text,
      color: PaletteColor::TextPrimary,
    };
  }
  Line::muted(Icon::Folder, text)
}

/// Local date and time of `at`, e.g. "Oct 3, 16:08".
pub fn day_and_clock(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  let (month, day) = display::month_day(at);
  let month = ctx.t_ns(NAMESPACE, MONTH_KEYS[month]);
  ctx.t_args(
    "summary.runs.when",
    [
      ("month", month.to_string()),
      ("day", day.to_string()),
      ("time", display::clock(at)),
    ],
  )
}
