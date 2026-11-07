//! The top of the review page (09i `wLyFt`): the run's title with *Open task* and *Approve and merge*, and the status
//! line (`Status` `tKlje`): a 14 glyph and *Waiting for your review* in 13/500 `info`, then the run, its steps, what it
//! changes, its head and how it merges in 12 `text_muted`; while Implement works on comments sent to it, *With the
//! agent* and what it works on (`agent`). *Approve and merge* merges at the head shown; it is not shown while Implement
//! works, and its reason stands before it. Once the branch moved past the head reviewed, the status reads *Changed
//! since your review* in `warning` with how many commits came after it and how much they change (`QCV81` in 05
//! `e6KEO`); the strip under it is [`super::since`]. Once the run was approved ([`approved`]) there is no *Approve and
//! merge*, and the status line tells how it merged.
//! A merge that conflicts has no *Approve and merge* and no other strip: its line says *Conflicts with master*,
//! approved, nothing merged and how many files conflict.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{project::TaskKey, revision::Revision};
use orchester_workspace::{
  display::{clock, short_revision},
  tasks::{
    changes::Checkpointed,
    review_threads::{ReviewRefusal, conflicts::ReviewConflicts, page::ReviewPage},
  },
};

use super::{Env, actions::Write, agent::Working, approved, conflict, since::moved};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  screens::merges::{attempt::head::title, words::joined},
  theme::{ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    unavailable::reason_line,
  },
};

/// The title with *Open task* and *Approve and merge*, which is not shown while Implement works on comments sent to it.
pub(super) fn header(ctx: &mut Ctx, env: &Env) -> Element {
  let open = open_task(ctx, &env.navigation, &env.project, &env.page.run.key);
  if env.read_only || env.review.implement_at_work || env.page.conflicts.is_some() {
    return title(&env.page.run.title, vec![open.into()]);
  }
  let approve = approve_button(ctx);
  let (approve, reason) = match unavailable(ctx, env) {
    Some(reason) => (approve.unavailable(reason.clone()), Some(reason)),
    None if env.writes.busy() => (approve.disabled(), None),
    None => {
      let (writes, head) = (env.writes.clone(), env.head.clone());
      (approve.on_click(move || writes.run(Write::Approve(head.clone()))), None)
    }
  };
  let mut actions: Vec<Element> = Vec::new();
  actions.extend(reason.map(|reason| {
    reason_line("review-approve-reason", &reason)
      .width(Dimension::Auto)
      .into()
  }));
  actions.push(open.into());
  actions.push(approve.into());
  title(&env.page.run.title, actions)
}

/// *Open task*, which opens the task `key` of `project`.
pub(super) fn open_task(ctx: &Ctx, navigation: &Navigation, project: &ShownProject, key: &TaskKey) -> ActionButton {
  ActionButton::new("review-open-task", ButtonKind::Secondary, ctx.t("review_page.openTask"))
    .icon(Icon::ArrowUpRight)
    .opens(navigation.to(Route::Project(
      project.open.project.id(),
      ProjectPage::Task(key.clone(), TaskView::of(TaskTab::Summary)),
    )))
}

/// *Approve and merge*, before it is told whether it can be pressed.
pub(super) fn approve_button(ctx: &Ctx) -> ActionButton {
  ActionButton::new("review-approve", ButtonKind::Primary, ctx.t("review_page.approve")).icon(Icon::Check)
}

/// Why *Approve and merge* cannot be pressed now.
fn unavailable(ctx: &Ctx, env: &Env) -> Option<Arc<str>> {
  if env.page.review_in.is_none() {
    return Some(message_text(ctx, &ReviewRefusal::ImplementAtWork.message()));
  }
  None
}

/// Where the run stands, which the status line's glyph and words say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Standing {
  AtWork,
  Changed,
  Waiting,
}

impl Standing {
  fn parts(self, ctx: &Ctx) -> (Icon, PaletteColor, Arc<str>, &'static str) {
    match self {
      Self::AtWork => (
        Icon::LoaderCircle,
        PaletteColor::Info,
        ctx.t("review_page.status.atWork"),
        "implement_at_work",
      ),
      Self::Changed => (
        Icon::GitCommitHorizontal,
        PaletteColor::Warning,
        ctx.t("review_page.status.changed"),
        "changed_since_review",
      ),
      Self::Waiting => (
        Icon::Eye,
        PaletteColor::Info,
        ctx.t("review_page.status.waiting"),
        "waiting_for_review",
      ),
    }
  }
}

/// The status line.
pub(super) fn status(ctx: &Ctx, env: &Env) -> Element {
  if let Some(approved) = &env.page.approved {
    return approved::status(ctx, env, approved);
  }
  if let Some(conflicts) = &env.page.conflicts {
    return conflicting(ctx, conflicts);
  }
  let standing = if env.page.review_in.is_none() || env.review.implement_at_work {
    Standing::AtWork
  } else if moved(&env.page).is_some() {
    Standing::Changed
  } else {
    Standing::Waiting
  };
  let working = Working::of(env);
  let meta = match &working {
    Some(working) => joined(ctx, &[working.status(ctx), working.sent(ctx)]),
    None => meta(ctx, env),
  };
  status_line(ctx, standing, &meta)
}

/// The status line of a run that stands as `standing`, with `meta` after it; the page's and its reading state's.
pub(super) fn status_line(ctx: &Ctx, standing: Standing, meta: &str) -> Element {
  let (glyph, color, words, name) = standing.parts(ctx);
  Row::new()
    .id("review-status")
    .describe("status", name)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, color.clone()))
    .child(
      Text::new(&words)
        .id("review-status-text")
        .variant(ExtraTypography::Strong.role())
        .color(color)
        .nowrap(),
    )
    .child(
      Text::new(meta)
        .id("review-status-meta")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// The status line of a merge that conflicts (`Status` `oTMra` in 11 `CT6Gc`): *Conflicts with master* in `danger`,
/// then when it was approved, that nothing merged and how many files conflict.
fn conflicting(ctx: &Ctx, conflicts: &ReviewConflicts) -> Element {
  let base = conflict::base_name(ctx, conflicts);
  let files = conflicts.files.len();
  let words = ctx.t_args("review_page.status.conflicts", [("base", &*base)]);
  let meta = ctx.t_args(
    if files == 1 {
      "review_page.status.conflictsMetaOne"
    } else {
      "review_page.status.conflictsMetaOther"
    },
    [("time", clock(conflicts.at)), ("count", files.to_string())],
  );
  Row::new()
    .id("review-status")
    .describe("status", "conflicts")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::GitCompareArrows, IconSize::Md, PaletteColor::Danger))
    .child(
      Text::new(&words)
        .id("review-status-text")
        .variant(ExtraTypography::Strong.role())
        .color(PaletteColor::Danger)
        .nowrap(),
    )
    .child(
      Text::new(&meta)
        .id("review-status-meta")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// *run #4 · every step passed · 7 files, +210 −36 · head 7c41e2b · into master*; once the branch moved past the head
/// reviewed, *2 commits after 0d6969d · 4 files, +88 −12 · into master*: the files and lines of the diff shown.
fn meta(ctx: &Ctx, env: &Env) -> Arc<str> {
  let page = &env.page;
  let mut parts = match moved(page) {
    Some(reviewed) => vec![commits_after(ctx, page, reviewed)],
    None => run_and_steps(ctx, page),
  };
  if let Some(size) = env.shown.size(page) {
    parts.push(size_words(ctx, size));
  }
  if moved(page).is_none() {
    parts.push(ctx.t_args("review_page.status.head", [("head", short_revision(env.head.as_str()))]));
  }
  if let Some(branch) = &page.run.base {
    parts.push(ctx.t_args("review_page.status.into", [("branch", branch.as_str())]));
  }
  joined(ctx, &parts)
}

/// *run #4* and how its steps went.
fn run_and_steps(ctx: &Ctx, page: &ReviewPage) -> Vec<Arc<str>> {
  let mut parts = vec![ctx.t_args("review_page.status.run", [("run", page.run.number.get().to_string())])];
  let (passed, total) = page.steps;
  parts.push(if passed == total {
    ctx.t("review_page.status.stepsPassed")
  } else {
    ctx.t_args(
      "review_page.status.stepsOf",
      [("passed", passed.to_string()), ("total", total.to_string())],
    )
  });
  parts
}

/// *2 commits after 0d6969d*.
fn commits_after(ctx: &Ctx, page: &ReviewPage, reviewed: &Revision) -> Arc<str> {
  let reviewed = short_revision(reviewed.as_str()).to_owned();
  match page.since_review.len() {
    1 => ctx.t_args("review_page.status.commitsAfterOne", [("reviewed", reviewed)]),
    count => ctx.t_args(
      "review_page.status.commitsAfterOther",
      [("count", count.to_string()), ("reviewed", reviewed)],
    ),
  }
}

/// *7 files, +210 −36*.
pub(super) fn size_words(ctx: &Ctx, (files, added, removed): (usize, u32, u32)) -> Arc<str> {
  let args = [
    ("files", files.to_string()),
    ("added", added.to_string()),
    ("removed", removed.to_string()),
  ];
  if files == 1 {
    ctx.t_args("review_page.status.sizeOne", args)
  } else {
    ctx.t_args("review_page.status.sizeOther", args)
  }
}

/// How many files the diff changes, and the lines it adds and removes.
pub(super) fn size(changes: &Checkpointed) -> Option<(usize, u32, u32)> {
  match changes {
    Checkpointed::Read { changes, .. } => Some((
      changes.diff.files().len(),
      changes.diff.additions(),
      changes.diff.deletions(),
    )),
    Checkpointed::TooLarge { size, .. } => Some((
      usize::try_from(size.files).unwrap_or(usize::MAX),
      size.additions,
      size.deletions,
    )),
    Checkpointed::NoCheckpoint => None,
  }
}
