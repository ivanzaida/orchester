//! The top card of a run that waits to be merged (`Decision · Ready to merge`: `wRsU7` in 01i `k9GBy`, `g03VoM` in 01j
//! `k4C0Es`): on `surface_raised`, radius 12, 14 by 16 inside and 10 between its parts; its head (a 14 glyph, the title
//! in 13/600 and a meta line in 12 `text_muted`, 8 apart), one line in 12.5/1.5 `text_secondary`, and its actions 8
//! apart. While the run waits to be merged it is `success` and `git-merge`, *Ready to merge*, the task, *every step
//! passed* and the run, then how much the run changes onto its branch. Once a merge did not land it is `warning` and
//! `triangle-alert`: the outcome as the run's attempt page titles it, the task, the stage it stopped at and when, then
//! the attempt in one line as the attempt page says it. Its actions: the merge's
//! (`crate::screens::task::merge`); *Review changes* (`file-diff`), or *Open in Review*
//! (`arrow-up-right`) once a merge did not land, which open the run's merge attempts under its task, its review page
//! before any attempt and its attempt page after; *Close without merging* (`circle-check-big`), which marks the task
//! done as *More actions* does; then *Review PR* once the run has a pull request. Under them why the merge waits, why
//! the page's own work on it did not happen, and what came of creating the run's pull request
//! ([`super::pull_request`]). *Merged into main* (`nqp68`): where the work went and what was cleared after it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::{Leftovers, MergeKind, RunMerge, attempt::AttemptOutcome};
use orchester_workspace::{
  display,
  messages::Message,
  tasks::{changes::Checkpointed, close::Closing, detail::RunView},
};

use super::{
  super::TaskContext,
  changed,
  pull_request::{self, PullRequestRead},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  screens::task::{
    PageState,
    ci::actions::open_page,
    header,
    merge::{
      self, Merge, MergeAction,
      attempt::{
        lines::{attempt_line, stage, when},
        standing::Standing,
      },
    },
  },
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

/// A conflicting file's row (`Q2ISvw`), in the merge waiting in the run's worktree.
pub(super) const FILE_ROW_HEIGHT: f32 = 30.0;
/// The branch a line names when the run did not record the one it merges into.
const NO_BRANCH: &str = "—";

/// How the card looks: its glyph, and the tone of the glyph and the outline.
struct Look {
  glyph: Icon,
  tone: PaletteColor,
  name: &'static str,
}

impl Look {
  /// The run's work passed: completing it (ORC-93, `Decision · Complete the task?` `YEOlP`).
  const PASSED: Self = Self {
    glyph: Icon::CircleCheck,
    tone: PaletteColor::Success,
    name: "success",
  };
  const READY: Self = Self {
    glyph: Icon::GitMerge,
    tone: PaletteColor::Success,
    name: "success",
  };
  const NOT_MERGED: Self = Self {
    glyph: Icon::TriangleAlert,
    tone: PaletteColor::Warning,
    name: "warning",
  };
}

/// What the card shows: how the run's merge stands, the run, and what is known of its pull request.
pub struct Shown<'a> {
  pub merge: Merge,
  pub run: &'a RunView,
  pub pull: &'a PullRequestRead,
}

/// The top card as `shown` stands, `changes` being the run's changes as read.
pub fn top(
  ctx: &mut Ctx,
  task: &TaskContext,
  shown: &Shown<'_>,
  changes: Option<&Result<Checkpointed, Message>>,
) -> Element {
  let Some(action) = merge::action(ctx, task, shown.merge) else {
    return Column::new().into();
  };
  let (look, words) = match shown.merge {
    Merge::Ready => (Look::READY, ready_words(ctx, task, shown.run, changes)),
    Merge::NotMerged(standing) => (Look::NOT_MERGED, not_merged_words(ctx, task, shown.run, standing)),
  };
  let pull = pull_request::parts(ctx, task, shown.run, shown.pull);
  let actions = buttons(ctx, task, shown, (&action, pull.review));
  let under = under(ctx, &action).into_iter().chain(pull.outcome).collect();
  framed(look, words, actions, under)
}

/// The card of a run whose work passed with `title` and `body` (`Decision · Complete the task?` `YEOlP`).
pub(in super::super) fn frame((title, body): (Arc<str>, Arc<str>), actions: Element, reasons: Vec<Element>) -> Element {
  framed(Look::PASSED, (title, None, body), actions, reasons)
}

/// The card in `look`: its head with `title` and `meta`, `body`, its `actions`, and `under` them what they say.
fn framed(
  look: Look,
  (title, meta, body): (Arc<str>, Option<Arc<str>>, Arc<str>),
  actions: Element,
  under: Vec<Element>,
) -> Element {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(look.glyph, IconSize::Md, look.tone.clone()))
    .child(
      Text::new(&title)
        .id("task-notice-title")
        .variant(TaskText::SectionTitle.role())
        .nowrap(),
    )
    .with_children(meta.map(|meta| {
      Text::new(&meta)
        .id("task-notice-meta")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(lurq::components::TextOverflow::Elipsis)
    }));
  Column::new()
    .id("task-notice")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, look.tone))
    .rounded(RadiusSize::Lg)
    .describe("tone", look.name)
    .child(head)
    .child(
      Text::new(&body)
        .id("task-notice-body")
        .variant(ExtraTypography::Lead.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(actions)
    .with_children(under)
    .into()
}

/// *Ready to merge*, *KON-12 · every step passed · run 8*, and how much the run changes onto its branch once that was
/// read.
fn ready_words(
  ctx: &Ctx,
  task: &TaskContext,
  run: &RunView,
  changes: Option<&Result<Checkpointed, Message>>,
) -> (Arc<str>, Option<Arc<str>>, Arc<str>) {
  let meta = ctx.t_args(
    "decisions.ready.meta",
    [
      ("key", task.key.to_string()),
      ("run", run.run.number().get().to_string()),
    ],
  );
  let body = match (run.run.base_branch(), changed::counts(changes)) {
    (Some(branch), Some((files, (added, removed)))) => {
      let key = if files == 1 {
        "decisions.ready.bodyOne"
      } else {
        "decisions.ready.bodyOther"
      };
      ctx.t_args(
        key,
        [
          ("files", files.to_string()),
          ("added", added.to_string()),
          ("removed", removed.to_string()),
          ("branch", branch.as_str().to_owned()),
        ],
      )
    }
    _ => ctx.t("decisions.ready.body"),
  };
  (ctx.t("decisions.ready.title"), Some(meta), body)
}

/// The outcome as the run's attempt page titles it, *KON-12 · refused at Integration lock · 12:41*, and the last
/// attempt in one line, as the attempt page says it.
fn not_merged_words(
  ctx: &Ctx,
  task: &TaskContext,
  run: &RunView,
  standing: Standing,
) -> (Arc<str>, Option<Arc<str>>, Arc<str>) {
  let branch = run.run.base_branch().map_or(NO_BRANCH, |branch| branch.as_str());
  let title = standing.status(ctx, branch);
  let Some(last) = task.attempt.as_deref().and_then(|page| page.last()) else {
    return (title, None, ctx.t("decisions.ready.notMerged.unknown"));
  };
  let attempt = &last.attempt;
  let (meta, at) = match attempt.outcome() {
    AttemptOutcome::Refused { stage: at } => ("decisions.ready.notMerged.refusedAt", Some(*at)),
    AttemptOutcome::GitStopped { stage: at } => ("decisions.ready.notMerged.stoppedAt", Some(*at)),
    AttemptOutcome::Interrupted { stage: at } => ("decisions.ready.notMerged.endedAt", Some(*at)),
    _ => ("decisions.ready.notMerged.at", None),
  };
  let meta = ctx.t_args(
    meta,
    [
      ("key", task.key.to_string()),
      ("stage", at.map(|at| stage(ctx, at).to_string()).unwrap_or_default()),
      ("time", when(ctx, attempt.at()).to_string()),
    ],
  );
  (title, Some(meta), attempt_line(ctx, attempt, branch))
}

/// The merge's action, *Merge and deliver* beside it for a team task's run on its module's branch
/// ([`merge::deliver`]), *Review changes* or *Open in Review*, *Close without merging*, and *Review PR* once the run
/// has pull request page `review`.
fn buttons(
  ctx: &mut Ctx,
  task: &TaskContext,
  shown: &Shown<'_>,
  (action, review): (&MergeAction, Option<Arc<str>>),
) -> Element {
  let navigation = Navigation::of(ctx);
  let project = task.project.open.project.id();
  let attempts = ProjectPage::TaskMerge(task.key.clone(), shown.run.run.number());
  let (id, label, glyph, page) = match shown.merge {
    Merge::Ready => (
      "task-review-changes",
      "decisions.ready.reviewChanges",
      Icon::FileDiff,
      ProjectPage::TaskReview(task.key.clone()),
    ),
    Merge::NotMerged(_) => (
      "task-open-in-review",
      "decisions.ready.openInReview",
      Icon::ArrowUpRight,
      attempts,
    ),
  };
  let open = ActionButton::new(id, ButtonKind::Secondary, ctx.t(label))
    .icon(glyph)
    .opens(navigation.to(Route::Project(project, page)));
  let deliver = merge::deliver(ctx, task, shown.merge).map(|deliver| deliver.control.button());
  Row::new()
    .id("task-ready-actions")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(action.control.button())
    .with_children(deliver)
    .child(open)
    .child(close(ctx, task))
    .with_children(review.map(|url| review_pull_request(ctx, &url)))
    .into()
}

/// *Review PR*: opens the run's pull request, at `url` on its repository's host, in the browser.
fn review_pull_request(ctx: &mut Ctx, url: &Arc<str>) -> ActionButton {
  ActionButton::new(
    "task-review-pull-request",
    ButtonKind::Secondary,
    ctx.t("decisions.ready.review"),
  )
  .icon(Icon::GitPullRequest)
  .on_click(open_page(&Services::of(ctx), url.clone()))
}

/// *Close without merging*: the task is marked done as *More actions* › *Mark as done* does, its run's work left
/// unmerged on its branch; under way while the task closes.
fn close(ctx: &mut Ctx, task: &TaskContext) -> ActionButton {
  let closer = PageState::of(ctx).closer;
  let button = ActionButton::new(
    "task-close-without-merging",
    ButtonKind::Ghost,
    ctx.t("decisions.ready.close"),
  )
  .icon(Icon::CircleCheckBig);
  if closer.busy() {
    return button.loading();
  }
  let subject = header::subject(task);
  button.on_click(move || closer.close(subject.clone(), Closing::Done))
}

/// Why the merge's action cannot be pressed now, and why the page's own work on the merge did not happen.
fn under(ctx: &mut Ctx, action: &MergeAction) -> Vec<Element> {
  let why = action
    .waits
    .clone()
    .or_else(|| action.control.reason().map(|reason| reason.full.clone()))
    .map(|why| reason_line("task-merge-reason", &why).into());
  let failed = PageState::of(ctx).merge.failed.get().map(|reason| {
    let words = ctx.t_args("decisions.ready.notDone", [("reason", message_text(ctx, &reason))]);
    field_error("task-merge-failed", &words).into()
  });
  why.into_iter().chain(failed).collect()
}

/// *Merged into main*: where the run's work went and what was cleared after it.
pub fn merged(ctx: &Ctx, task: &TaskContext, merge: &RunMerge) -> Element {
  let key = match merge.kind {
    MergeKind::FastForward => "decisions.ready.merged.fastForward",
    MergeKind::MergeCommit => "decisions.ready.merged.mergeCommit",
    MergeKind::AlreadyMerged => "decisions.ready.merged.alreadyMerged",
    MergeKind::Squash => "decisions.ready.merged.squash",
  };
  let branch = merge.into.as_str();
  let moved = ctx.t_args(
    key,
    [
      ("branch", branch),
      ("revision", display::short_revision(merge.revision.as_str())),
      ("time", display::clock(merge.merged_at).as_str()),
    ],
  );
  let left = task
    .left
    .as_read(merge.run)
    // Why what the run left could not be read is said in its row of *Runs of this task*.
    .and_then(Result::ok)
    .map(|left| left_behind(ctx, &left));
  let body = match left {
    Some(left) => ctx.t_args("decisions.ready.sentences", [("first", &*moved), ("second", &*left)]),
    None => moved,
  };
  Notice::new(
    "task-notice",
    NoticeTone::Success,
    ctx.t_args("decisions.ready.mergedTitle", [("branch", branch)]),
  )
  .icon(Icon::GitMerge)
  .body(body)
  .into()
}

/// What is left of the run after its merge: nothing, worktrees kept for their uncommitted changes,
/// branches whose commits are not merged, or merged branches that are still there.
fn left_behind(ctx: &Ctx, left: &Leftovers) -> Arc<str> {
  let separator = ctx.t("task.listSeparator");
  let names = |merged: bool| {
    left
      .branches
      .iter()
      .filter(|branch| branch.merged == merged)
      .map(|branch| branch.name.as_str())
      .collect::<Vec<_>>()
      .join(&separator)
  };
  let (unmerged, merged) = (names(false), names(true));
  let staying = left.staying();
  match (staying.with_changes, staying.unfinished) {
    (1, 0) => ctx.t("decisions.ready.left.worktreeOne"),
    (count, 0) if count > 1 => ctx.t_args("decisions.ready.left.worktreeOther", [("count", count.to_string())]),
    (0, 1) => ctx.t("decisions.ready.left.unfinishedOne"),
    (with_changes, unfinished) if with_changes + unfinished > 0 => ctx.t_args(
      "decisions.ready.left.unfinishedOther",
      [("count", (with_changes + unfinished).to_string())],
    ),
    _ if !unmerged.is_empty() => ctx.t_args("decisions.ready.left.branches", [("branches", unmerged)]),
    _ if !merged.is_empty() => ctx.t_args("decisions.ready.left.mergedBranches", [("branches", merged)]),
    _ => ctx.t("decisions.ready.left.nothing"),
  }
}

/// The merge's action while a merge waits in the run's worktree for the person to resolve it, which it waits for,
/// with why; *Review PR* beside it once the run has a pull request.
pub fn merge_actions(ctx: &mut Ctx, task: &TaskContext, run: &RunView, pull: &PullRequestRead) -> Element {
  let Some(standing) = Merge::of(task) else {
    return Column::new().into();
  };
  let Some(action) = merge::action(ctx, task, standing) else {
    return Column::new().into();
  };
  let pull = pull_request::parts(ctx, task, run, pull);
  let review = pull.review.map(|url| review_pull_request(ctx, &url));
  let row = Row::new()
    .id("task-ready-actions")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(action.control.button())
    .with_children(review);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(row)
    .with_children(under(ctx, &action))
    .with_children(pull.outcome)
    .into()
}
