//! Where the run's work stands and what can be done with it. The Ready card (`Ready card` `hyzq2`): ready
//! for review, *Merge into main*, *Create pull request* ([`super::pull_request`]), *Ask agent* and *Review PR*,
//! and why the ones that cannot be used are unavailable (`Unavailable reason` `QR1ER`). *Merged into main*
//! (`nqp68`): where the work went and what was cleared after it. *Not merged into main* (`D3kggY`): why, the
//! files that conflicted (`Conflicts` `IBx7h`), and the same actions to merge again.

use std::{num::NonZeroU64, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::{Leftovers, MergeKind, RunMerge};
use orchester_workspace::{
  display,
  messages::Message,
  tasks::{detail::RunView, integration_jobs::JobState},
};

use super::{
  super::{super::section_head, TaskContext},
  pull_request::{self, PullRequestRead},
};
use crate::{
  theme::{DecisionSpacing, DecisionText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

/// A conflicting file's row (`Q2ISvw`).
const FILE_ROW_HEIGHT: f32 = 30.0;

/// The Ready card of a run that is not merged.
pub fn ready(ctx: &mut Ctx, task: &TaskContext, run: &RunView, pull: &PullRequestRead) -> Element {
  let (actions, reasons) = actions(ctx, task, run, pull);
  let text = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(&ctx.t("decisions.ready.title"))
        .id("task-notice-title")
        .variant(DecisionText::ReadyTitle.role()),
    )
    .child(Text::new(&ctx.t("decisions.ready.body")).variant(ExtraTypography::Lead.role()));
  Column::new()
    .id("task-notice")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(DecisionSpacing::ReadySide.role(), SpacingSize::Lg))
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SuccessMuted)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Success))
    .rounded(RadiusSize::Lg)
    .describe("tone", "success")
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Start)
        .child(icon(Icon::CircleCheck, IconSize::Lg, PaletteColor::Success))
        .child(text),
    )
    .child(actions)
    .with_children(reasons)
    .into()
}

/// *Merged into main*: where the run's work went and what was cleared after it.
pub fn merged(ctx: &Ctx, task: &TaskContext, merge: &RunMerge) -> Element {
  let key = match merge.kind {
    MergeKind::FastForward => "decisions.ready.merged.fastForward",
    MergeKind::MergeCommit => "decisions.ready.merged.mergeCommit",
    MergeKind::AlreadyMerged => "decisions.ready.merged.alreadyMerged",
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
  match left.worktrees.len() {
    1 => ctx.t("decisions.ready.left.worktreeOne"),
    count if count > 1 => ctx.t_args("decisions.ready.left.worktreeOther", [("count", count.to_string())]),
    _ if !unmerged.is_empty() => ctx.t_args("decisions.ready.left.branches", [("branches", unmerged)]),
    _ if !merged.is_empty() => ctx.t_args("decisions.ready.left.mergedBranches", [("branches", merged)]),
    _ => ctx.t("decisions.ready.left.nothing"),
  }
}

/// Why the last merge did not stand: *Not merged into main* with why and the files that conflicted, or
/// that git merged the run (`made`) but the merge could not be recorded; then the actions to merge again,
/// which records a merge git made.
pub fn not_merged(
  ctx: &mut Ctx,
  task: &TaskContext,
  run: &RunView,
  (problem, made): (&Message, Option<&RunMerge>),
  (conflicts, pull): (&[String], &PullRequestRead),
) -> Vec<Element> {
  let title = match (made, run.run.base_branch()) {
    (Some(made), _) => ctx.t_args("decisions.ready.notRecordedTitle", [("branch", made.into.as_str())]),
    (None, Some(branch)) => ctx.t_args("decisions.ready.notMergedTitle", [("branch", branch.as_str())]),
    (None, None) => ctx.t("decisions.ready.notMergedNoBranch"),
  };
  let notice = Notice::new("task-notice", NoticeTone::Warning, title)
    .icon(Icon::TriangleAlert)
    .body(message_text(ctx, problem));
  let files = (!conflicts.is_empty()).then(|| conflicting(ctx, conflicts));
  let (actions, reasons) = actions(ctx, task, run, pull);
  let actions = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(actions)
    .with_children(reasons);
  std::iter::once(notice.into())
    .chain(files)
    .chain(std::iter::once(actions.into()))
    .collect()
}

/// *Conflicting files* with their count, a row per file.
fn conflicting(ctx: &mut Ctx, files: &[String]) -> Element {
  let rows = files.iter().map(|file| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FILE_ROW_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::GitCompareArrows, IconSize::Base, PaletteColor::Warning))
      .child(Text::new(file).variant(TypographyStyle::Mono).flex(1.0).nowrap())
  });
  let list = card("task-merge-conflicts", CardSurface::Outline)
    .padding(Padding::symmetric(ExtraSpacing::Snug.role(), ExtraSpacing::Hair.role()))
    .with_children(rows);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(section_head(
      ctx,
      "task-merge-conflicts-head",
      "decisions.ready.conflicts",
      Some(files.len()),
    ))
    .child(list)
    .into()
}

/// *Merge into main*, *Create pull request* with *Open as draft*, *Ask agent* and *Review PR* (`XKJdW`); what
/// came of creating a pull request; and why the actions that cannot be used now are unavailable.
fn actions(ctx: &mut Ctx, task: &TaskContext, run: &RunView, pull: &PullRequestRead) -> (Element, Vec<Element>) {
  let (merge, merge_reason) = merge_button(ctx, task, run);
  let pull = pull_request::parts(ctx, task, run, pull);
  let ask_reason = ctx.t("decisions.ready.askUnavailable");
  let ask = ActionButton::new("task-ask-agent", ButtonKind::Secondary, ctx.t("decisions.ready.ask"))
    .icon(Icon::MessageSquareText)
    .unavailable(ask_reason.clone());
  let (review, review_reason) = review_button(ctx, task, pull.review);
  let row = Row::new()
    .id("task-ready-actions")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(merge)
    .with_children(pull.actions)
    .child(ask)
    .child(review);
  let reasons = merge_reason
    .map(|reason| ("task-merge-reason", reason))
    .into_iter()
    .chain(pull.reason.map(|reason| ("task-create-pull-request-reason", reason)))
    .chain([("task-ask-agent-reason", ask_reason)])
    .chain(review_reason.map(|reason| ("task-review-pull-request-reason", reason)))
    .map(|(id, reason)| reason_line(id, &reason).into());
  (row.into(), pull.outcome.into_iter().chain(reasons).collect())
}

/// *Review PR*: opens the run's pull request, unavailable until it has one.
fn review_button(ctx: &mut Ctx, task: &TaskContext, number: Option<NonZeroU64>) -> (ActionButton, Option<Arc<str>>) {
  let review = ActionButton::new(
    "task-review-pull-request",
    ButtonKind::Secondary,
    ctx.t("decisions.ready.review"),
  )
  .icon(Icon::GitPullRequest);
  let Some(number) = number else {
    let reason = ctx.t("decisions.ready.reviewUnavailable");
    return (review.unavailable(reason.clone()), Some(reason));
  };
  let navigation = crate::navigation::Navigation::of(ctx);
  let route = crate::routes::Route::Project(
    task.project.open.project.id(),
    crate::routes::ProjectPage::PullRequest(number, None),
  );
  (review.opens(navigation.to(route)), None)
}

/// *Merge into main*: under way while git merges, waiting while the run's worktree is removed, its pull request
/// is created or another run of the project merges, and unavailable with why when the run did not record the
/// branch it started from.
fn merge_button(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> (ActionButton, Option<Arc<str>>) {
  let Some(branch) = run.run.base_branch() else {
    let reason = ctx.t("decisions.ready.mergeNoBranch");
    let button = ActionButton::new("task-merge", ButtonKind::Primary, ctx.t("decisions.ready.mergeUnknown"))
      .icon(Icon::GitMerge)
      .unavailable(reason.clone());
    return (button, Some(reason));
  };
  let job = task.jobs.run(run.id());
  let label = ctx.t_args("decisions.ready.merge", [("branch", branch.as_str())]);
  let button = ActionButton::new("task-merge", ButtonKind::Primary, label).icon(Icon::GitMerge);
  if job.merge == JobState::Running {
    return (button.loading(), None);
  }
  if task.jobs.merging().is_some_and(|other| other != run.id()) {
    return (button.disabled(), Some(ctx.t("decisions.ready.anotherMerge")));
  }
  if job.creating_pull_request() {
    return (button.disabled(), Some(ctx.t("decisions.ready.mergeWaitsPullRequest")));
  }
  if job.busy() {
    return (button.disabled(), Some(ctx.t("decisions.ready.mergeWaits")));
  }
  let navigation = crate::navigation::Navigation::of(ctx);
  let route = crate::routes::Route::Project(
    task.project.open.project.id(),
    task.page(crate::routes::TaskView::LocalIntegration),
  );
  (button.opens(navigation.to(route)), None)
}
