//! *Waiting for your review* (`Section · Waiting for your review` `c7rBsw` in 09 `Os7b3`): a row per run that passed
//! every step and merges only once a person approves it, *Ready for review* on its task page or *Waiting for approval*
//! in the merge queue, with its `info` eye. Its line says what it changes, how it merges and the review comments not
//! sent yet; once its branch moved after the head that was reviewed, *Changed since your review: 2 commits after
//! 0d6969d* in `warning` (`wVIYy` in `Row · KON-20` `cd7dN`). The row, and its *Review changes* or *Review again*, open
//! the run's review page (ORC-697, [`super::review`]).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_core::integration::LoggedCommit;
use orchester_workspace::{
  display::{clock, short_revision},
  tasks::merge_attempts::{
    merges::WaitingRun,
    read::{ReviewIn, SinceReview},
  },
};

use super::{
  attempt_page,
  row::{self, Shown},
  words,
};
use crate::{
  project::ShownProject,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// The section, or nothing without a run waiting for review.
pub(super) fn section(ctx: &mut Ctx, project: &ShownProject, runs: &[WaitingRun]) -> Option<Element> {
  if runs.is_empty() {
    return None;
  }
  let rows = runs
    .iter()
    .enumerate()
    .map(|(at, run)| {
      let shown = shown(ctx, project, run);
      row::row(ctx, &run.run, shown, at + 1 == runs.len())
    })
    .collect();
  Some(row::section("merges-waiting", &ctx.t("merges.section.waiting"), rows))
}

fn shown(ctx: &mut Ctx, project: &ShownProject, run: &WaitingRun) -> Shown {
  let id = format!("merges-waiting-{}", run.run.run);
  let moved = match &run.waiting.since_review {
    Some(SinceReview::Moved { reviewed, commits }) => Some(changed_line(ctx, reviewed.as_str(), commits)),
    Some(SinceReview::Unchanged) | None => None,
  };
  let changed = moved.is_some();
  let (line, color, name) = match moved {
    Some(line) => (line, PaletteColor::Warning, "changed_since_review"),
    None => (passed_line(ctx, run), PaletteColor::TextSecondary, "waiting_for_review"),
  };
  let since = [("time", clock(run.waiting.since))];
  let meta = match run.waiting.review_in {
    ReviewIn::MergeQueue => ctx.t_args("merges.waiting.inQueue", since),
    ReviewIn::TaskPage => ctx.t_args("merges.waiting.onTaskPage", since),
  };
  let key = if changed {
    "merges.action.reviewAgain"
  } else {
    "merges.action.reviewChanges"
  };
  let review = ActionButton::new(format!("{id}-action"), ButtonKind::Secondary, ctx.t(key))
    .icon(Icon::FileDiff)
    .small()
    .opens(attempt_page(ctx, project, &run.run));
  Shown {
    glyph: (Icon::Eye, PaletteColor::Info),
    line: (row::line(&id, &line, color), name),
    meta,
    action: Some(review.into()),
    opens: attempt_page(ctx, project, &run.run),
    id,
  }
}

/// *Changed since your review: 2 commits after 0d6969d · nothing merged · review again*; without the commits when git
/// could not list them, or the branch is gone.
fn changed_line(ctx: &Ctx, reviewed: &str, commits: &[LoggedCommit]) -> Arc<str> {
  let reviewed = short_revision(reviewed).to_owned();
  match commits.len() {
    0 => ctx.t("merges.waiting.changed"),
    1 => ctx.t_args("merges.waiting.changedOne", [("reviewed", reviewed)]),
    count => ctx.t_args(
      "merges.waiting.changedOther",
      [("count", count.to_string()), ("reviewed", reviewed)],
    ),
  }
}

/// *Every step passed · 7 files, +210 −36 · merges into master once you approve · 2 comments not sent*.
fn passed_line(ctx: &Ctx, run: &WaitingRun) -> Arc<str> {
  let mut parts = vec![ctx.t("merges.waiting.passed")];
  if let Some(size) = &run.size {
    let (added, removed) = (size.additions.to_string(), size.deletions.to_string());
    parts.push(if size.files == 1 {
      ctx.t_args("merges.waiting.sizeOne", [("added", added), ("removed", removed)])
    } else {
      ctx.t_args(
        "merges.waiting.sizeOther",
        [
          ("files", size.files.to_string()),
          ("added", added),
          ("removed", removed),
        ],
      )
    });
  }
  parts.push(match &run.run.base {
    Some(branch) => ctx.t_args("merges.waiting.mergesInto", [("branch", branch.as_str())]),
    None => ctx.t("merges.waiting.mergesOnce"),
  });
  match run.unsent {
    0 => {}
    1 => parts.push(ctx.t("merges.waiting.unsentOne")),
    count => parts.push(ctx.t_args("merges.waiting.unsentOther", [("count", count.to_string())])),
  }
  words::joined(ctx, &parts)
}
