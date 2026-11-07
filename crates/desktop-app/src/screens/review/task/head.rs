//! The top of the review page (09i `wLyFt`): the run's title with *Open task* and *Approve and merge*, and the status
//! line (`Status` `tKlje`): a 14 glyph and *Waiting for your review* in 13/500 `info`, then the run, its steps, what it
//! changes, its head and how it merges in 12 `text_muted`; while Implement works on comments sent to it, *With the
//! agent* and what it works on (`agent`). *Approve and merge* merges at the head shown; it is not shown while Implement
//! works, and unavailable once the branch moved past the head reviewed, and its reason stands before it. While comments
//! wait unsent it asks first in a modal ([`approve`]). Then the branch moved: *Changed since your review* says so and
//! lists the new commits, and the person takes them in before they approve. Once the run was approved ([`approved`])
//! there is no *Approve and merge*, and the status line tells how it merged.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{project::TaskKey, revision::Revision};
use orchester_workspace::{
  display::{clock, short_revision},
  tasks::{
    changes::Checkpointed,
    review_threads::{ReviewRefusal, page::ReviewPage},
  },
};

use super::{Env, actions::Write, agent::Working, approve, approved, state::Held, words::counted};
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
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

/// The head that was reviewed, when the branch moved past it since and the person did not take the head shown in.
pub(super) fn moved(env: &Env) -> Option<&Revision> {
  if env.read_only {
    return None;
  }
  let reviewed = env.page.reviewed.as_ref()?;
  let taken = env.state.taken_in.get().0;
  (*reviewed != env.head && taken.as_ref() != Some(&env.head)).then_some(reviewed)
}

/// The title with *Open task* and *Approve and merge*, which is not shown while Implement works on comments sent to it.
pub(super) fn header(ctx: &mut Ctx, env: &Env) -> Element {
  let open = open_task(ctx, &env.navigation, &env.project, &env.page.run.key);
  if env.read_only || env.review.implement_at_work {
    return title(&env.page.run.title, vec![open.into()]);
  }
  let button = approve_button(ctx);
  let (button, reason) = match unavailable(ctx, env) {
    Some(reason) => (button.unavailable(reason.clone()), Some(reason)),
    None if env.writes.busy() => (button.disabled(), None),
    None => {
      let (writes, head) = (env.writes.clone(), env.head.clone());
      let (asking, waiting) = (env.state.approving.clone(), !approve::unsent(env).is_empty());
      let press = move || {
        if waiting {
          asking.set(true);
        } else {
          writes.run(Write::Approve(head.clone(), Vec::new()));
        }
      };
      (button.on_click(press), None)
    }
  };
  let mut actions: Vec<Element> = Vec::new();
  actions.extend(reason.map(|reason| {
    reason_line("review-approve-reason", &reason)
      .width(Dimension::Auto)
      .into()
  }));
  actions.push(open.into());
  actions.push(Row::new().child(button).child(approve::dialog(ctx, env)).into());
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
  if moved(env).is_some() {
    return Some(ctx.t("review_page.changed.reason"));
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
        Icon::TriangleAlert,
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
  let standing = if env.page.review_in.is_none() || env.review.implement_at_work {
    Standing::AtWork
  } else if moved(env).is_some() {
    Standing::Changed
  } else {
    Standing::Waiting
  };
  let working = Working::of(env);
  let meta = match &working {
    Some(working) => joined(ctx, &[working.status(ctx), working.sent(ctx)]),
    None => meta(ctx, &env.page, &env.head),
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

/// *run #4 · every step passed · 7 files, +210 −36 · head 7c41e2b · into master*.
fn meta(ctx: &Ctx, page: &ReviewPage, head: &Revision) -> Arc<str> {
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
  if let Some(size) = size(&page.changes) {
    parts.push(size_words(ctx, size));
  }
  parts.push(ctx.t_args("review_page.status.head", [("head", short_revision(head.as_str()))]));
  if let Some(branch) = &page.run.base {
    parts.push(ctx.t_args("review_page.status.into", [("branch", branch.as_str())]));
  }
  joined(ctx, &parts)
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
fn size(changes: &Checkpointed) -> Option<(usize, u32, u32)> {
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

/// *Changed since your review*: the commits after the head reviewed, newest first, and *I've looked at them*, which
/// takes the head shown in.
pub(super) fn changed(ctx: &Ctx, env: &Env) -> Option<Element> {
  let reviewed = moved(env)?;
  let commits: Vec<Element> = env
    .page
    .since_review
    .iter()
    .map(|commit| {
      Text::new(&ctx.t_args(
        "review_page.changed.commit",
        [
          ("commit", short_revision(commit.revision.as_str()).to_owned()),
          ("subject", commit.subject.clone()),
          ("time", clock(commit.at)),
        ],
      ))
      .variant(TypographyStyle::Mono)
      .color(PaletteColor::TextSecondary)
      .nowrap()
      .into()
    })
    .collect();
  let count = env.page.since_review.len();
  let body = ctx.t_args(
    "review_page.changed.body",
    [
      ("reviewed", short_revision(reviewed.as_str()).to_owned()),
      ("head", short_revision(env.head.as_str()).to_owned()),
      (
        "commits",
        counted(
          ctx,
          ("review_page.changed.commitsOne", "review_page.changed.commitsOther"),
          count,
        )
        .to_string(),
      ),
    ],
  );
  let (taken, head) = (env.state.taken_in.clone(), env.head.clone());
  let take = ActionButton::new(
    "review-changed-take",
    ButtonKind::Secondary,
    ctx.t("review_page.changed.take"),
  )
  .icon(Icon::Check)
  .small()
  .on_click(move || taken.set(Held(Some(head.clone()))));
  Some(
    Notice::new(
      "review-changed",
      NoticeTone::Warning,
      ctx.t("review_page.changed.title"),
    )
    .icon(Icon::TriangleAlert)
    .body(body)
    .part(Column::new().id("review-changed-commits").with_children(commits))
    .action(take)
    .into(),
  )
}
