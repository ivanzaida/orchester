//! What a run's worktree holds (`Worktree` `uB3kM` in `Run #8` `Km4KK`): in a word or two at the row's end (*worktree
//! kept*, *worktree removed*), and in full in its tooltip: what the run's worktrees hold, that they were removed and
//! when, that they are in use, that the run never had one, or that nothing can be said while the run cannot be read.

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

/// What the run's worktree holds: its glyph, its words in short and in full, and their color.
pub struct Line {
  pub glyph: Icon,
  /// The key of its words in short.
  pub short: &'static str,
  pub text: Arc<str>,
  pub color: PaletteColor,
  /// The run's worktree is there to open.
  pub present: bool,
}

impl Line {
  const fn muted(glyph: Icon, short: &'static str, text: Arc<str>) -> Self {
    Self {
      glyph,
      short,
      text,
      color: PaletteColor::TextMuted,
      present: false,
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
    return Some(Line::muted(Icon::Lock, "summary.runs.short.unreadable", text));
  };
  if run.current_attempt().is_none() {
    let text = ctx.t("summary.runs.worktree.none");
    return Some(Line::muted(Icon::Minus, "summary.runs.short.none", text));
  }
  let branch = [("branch", run.branch().as_str())];
  if run.status().has_live_work() {
    let (key, short) = match run.status() {
      RunStatus::Paused(PauseReason::Requested) => ("summary.runs.worktree.keptForResume", "summary.runs.short.kept"),
      RunStatus::Running if run.pause_requested() => ("summary.runs.worktree.keptForResume", "summary.runs.short.kept"),
      RunStatus::Paused(PauseReason::LimitReached(_)) => {
        ("summary.runs.worktree.keptToContinue", "summary.runs.short.kept")
      }
      _ => ("summary.runs.worktree.inUse", "summary.runs.short.inUse"),
    };
    let mut line = Line::muted(Icon::FolderGit2, short, ctx.t_args(key, branch));
    line.present = true;
    return Some(line);
  }
  match left? {
    Err(error) => Some(Line::muted(
      Icon::CircleAlert,
      "summary.runs.short.unknown",
      message_text(ctx, error),
    )),
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
  Line::muted(Icon::FolderX, "summary.runs.short.removed", text)
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
  let missing = !several && left.worktrees.iter().all(|worktree| worktree.missing);
  let key = match (several, files) {
    (false, _) if missing => "summary.runs.worktree.missing",
    (false, 0) => "summary.runs.worktree.clean",
    (false, 1) => "summary.runs.worktree.uncommittedOne",
    (false, _) => "summary.runs.worktree.uncommittedOther",
    (true, 0) => "summary.runs.worktree.severalClean",
    (true, _) => "summary.runs.worktree.severalUncommitted",
  };
  let text = ctx.t_args(key, args);
  if missing {
    return Line::muted(Icon::FolderX, "summary.runs.short.missing", text);
  }
  Line {
    glyph: Icon::FolderGit2,
    short: "summary.runs.short.kept",
    text,
    color: if left.loses_files() {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextMuted
    },
    present: true,
  }
}

/// Local date and time of `at`, e.g. "Oct 3, 16:08", or the time alone today ("16:20").
pub fn day_and_clock(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  // Today's times go without the date.
  if at.local_day_start() == Timestamp::now().local_day_start() {
    return Arc::from(display::clock(at));
  }
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
