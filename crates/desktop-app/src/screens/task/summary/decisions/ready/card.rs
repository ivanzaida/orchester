//! Where the run's work stands and what can be done with it. The Ready card (`Ready card` `hyzq2`): ready
//! for review, *Merge into main*, *Create pull request* ([`super::pull_request`]), *Ask agent* and *Review PR*,
//! and why the ones that cannot be used are unavailable (`Unavailable reason` `QR1ER`). *Merge into main* merges the
//! run into the checkout's branch beside the person's uncommitted work, on the job `merge_run` uses, and after a merge
//! that did not land tries again (the attempt is on the run's attempt page, ORC-363). *Merged into main* (`nqp68`):
//! where the work went and what was cleared after it.

use std::{num::NonZeroU64, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::{Leftovers, MergeKind, RunMerge};
use orchester_workspace::{
  display,
  tasks::{
    detail::RunView,
    integration_jobs::{JobState, MergeArgs},
  },
};

use super::{
  super::TaskContext,
  pull_request::{self, PullRequestRead},
};
use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

/// A conflicting file's row (`Q2ISvw`), in the merge waiting in the run's worktree.
pub(super) const FILE_ROW_HEIGHT: f32 = 30.0;

/// The Ready card of a run that is not merged.
pub fn ready(ctx: &mut Ctx, task: &TaskContext, run: &RunView, pull: &PullRequestRead) -> Element {
  let (actions, reasons) = actions(ctx, task, run, (pull, false));
  frame(
    (ctx.t("decisions.ready.title"), ctx.t("decisions.ready.body")),
    actions,
    reasons,
  )
}

/// The Ready card's frame (`Decision · Ready to merge` `I500yD`): on `surface_raised` in a `success` stroke, its head
/// (the 14 check in `success` and its title in 13/600), what it means in 12.5 `text_secondary`, then its `actions` and
/// why those that cannot be used are unavailable (`reasons`). A run that committed nothing and is completed rather than
/// merged shows it too (ORC-93, `Decision · Complete the task?` `YEOlP`).
pub(in super::super) fn frame((title, body): (Arc<str>, Arc<str>), actions: Element, reasons: Vec<Element>) -> Element {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleCheck, IconSize::Md, PaletteColor::Success))
    .child(
      Text::new(&title)
        .id("task-notice-title")
        .variant(TaskText::SectionTitle.role()),
    );
  Column::new()
    .id("task-notice")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Success))
    .rounded(RadiusSize::Lg)
    .describe("tone", "success")
    .child(head)
    .child(
      Text::new(&body)
        .id("task-notice-body")
        .variant(ExtraTypography::Lead.role())
        .width(Dimension::Pct(100.0)),
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
  match left.worktrees.len() {
    1 => ctx.t("decisions.ready.left.worktreeOne"),
    count if count > 1 => ctx.t_args("decisions.ready.left.worktreeOther", [("count", count.to_string())]),
    _ if !unmerged.is_empty() => ctx.t_args("decisions.ready.left.branches", [("branches", unmerged)]),
    _ if !merged.is_empty() => ctx.t_args("decisions.ready.left.mergedBranches", [("branches", merged)]),
    _ => ctx.t("decisions.ready.left.nothing"),
  }
}

/// The actions to merge after a merge waiting `in_worktree` for the person to resolve it, with why those that cannot
/// be used now are unavailable; *Merge into main* waits while that merge does.
pub fn merge_actions(
  ctx: &mut Ctx,
  task: &TaskContext,
  run: &RunView,
  (pull, in_worktree): (&PullRequestRead, bool),
) -> Element {
  let (actions, reasons) = actions(ctx, task, run, (pull, in_worktree));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(actions)
    .with_children(reasons)
    .into()
}

/// *Merge into main*, *Create pull request* with *Open as draft*, *Ask agent* and *Review PR* (`XKJdW`); what
/// came of creating a pull request; and why the actions that cannot be used now are unavailable.
fn actions(
  ctx: &mut Ctx,
  task: &TaskContext,
  run: &RunView,
  (pull, in_worktree): (&PullRequestRead, bool),
) -> (Element, Vec<Element>) {
  let (merge, merge_reason) = merge_button(ctx, task, run, in_worktree);
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

/// *Merge into main*: merges the run on the project's shared operations, the job Orchester's `merge_run` uses. A
/// person's press tells them the outcome in full; an MCP client's press through the window goes through `merge_run`'s
/// policy (`RunJobs::start_synthetic_merge`), which refuses and redacts what the tool would. Under way while git
/// merges, waiting while the run's worktree is removed, its pull request is created, Local integration works in the
/// checkout, another run of the project merges or a decision of Local integration waits, a merge waits in the run's
/// worktree for the person to resolve it, and unavailable with why when the run did not record the
/// branch it started from.
fn merge_button(
  ctx: &mut Ctx,
  task: &TaskContext,
  run: &RunView,
  in_worktree: bool,
) -> (ActionButton, Option<Arc<str>>) {
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
  if job.removing() {
    return (button.disabled(), Some(ctx.t("decisions.ready.mergeWaits")));
  }
  if job.resolving() || in_worktree {
    return (button.disabled(), Some(ctx.t("decisions.ready.mergeResolving")));
  }
  if job.busy() {
    return (button.disabled(), Some(ctx.t("decisions.ready.mergeLocalWork")));
  }
  // A merge would not begin while a person answers Local integration (`RunJobs::local_decision`).
  if task.jobs.local_decision_pending() {
    return (button.disabled(), Some(ctx.t("decisions.ready.mergeDecision")));
  }
  let services = Services::of(ctx);
  let (jobs, vault) = (
    services.jobs.of(task.project.open.project.id()).of(run.id()),
    services.vault,
  );
  let args = merge_args(ctx, task, run, branch.as_str());
  // A merge that does not begin (another one took the project meanwhile) leaves the job's state, which
  // the card shows, as it is.
  let start = move || {
    if lurq::app::events::is_synthetic_input() {
      jobs.start_synthetic_merge(args.clone(), vault.clone());
    } else {
      jobs.start_merge(args.clone());
    }
  };
  (button.on_click(start), None)
}

/// The merge of `run` into `branch`, reported in the status bar; once it ends the task is read again.
fn merge_args(ctx: &mut Ctx, task: &TaskContext, run: &RunView, branch: &str) -> MergeArgs {
  let services = Services::of(ctx);
  let feed = task.project.feed.clone();
  MergeArgs {
    open: task.project.open.clone(),
    task: task.detail.task.id(),
    key: task.detail.task.key().to_string(),
    number: run.run.number().get(),
    branch: branch.to_owned(),
    run: run.id(),
    programs: services.programs,
    activity: services.activity,
    separator: ctx.t("task.listSeparator"),
    changed: Arc::new(move || feed.saved()),
  }
}
