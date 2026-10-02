//! Merging from Ready for review (02.P `GIiCb`, states `OP7dt`): *Merge into <branch>* is a
//! secondary button beside the unavailable *Review PR* (`o5Txa1`), in the review actions (`ksjis`).
//! It merges the run into the branch the user's checkout had when the run started, in that
//! checkout, then removes what the run left that is safe to remove (see
//! `orchester_core::integration`). A refusal or a conflict is a notice above the actions that says
//! why and that nothing changed (02.P2 `lDcdB`). Once merged, the readiness card (`IBuWf`) says
//! where the work went and what is left of the run (02.P3 `bMJUA`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::ElementRef,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::{Leftovers, MergeKind, RunMerge};

use crate::{
  activity::Activity,
  display,
  programs::Programs,
  tasks::{
    detail::TaskDetail,
    integration_jobs::{JobState, MergeArgs, RunIntegration},
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
  },
  workspace::OpenProject,
};

/// Height of the readiness card (`IBuWf`).
const READINESS_HEIGHT: f32 = 76.0;
/// Minimum width of Merge into main in the design (`gSsaS`); long names reserve their own width.
const MERGE_BUTTON_WIDTH: f32 = 140.0;

#[derive(Clone)]
pub(super) struct MergeAction {
  job: RunIntegration,
  start: Arc<dyn Fn() + Send + Sync>,
}

/// Merges the latest run of `detail`'s task in `open`'s checkout, reported in the status bar.
pub(super) fn merge_action(ctx: &mut Ctx, open: &OpenProject, detail: &TaskDetail) -> MergeAction {
  let job = ctx
    .use_context::<RunIntegration>()
    .expect("the task page provides its run integration");
  let nav = ctx
    .use_context::<super::super::super::ProjectNav>()
    .expect("the project shell provides ProjectNav");
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
  let activity = Activity::of(ctx);
  let separator = ctx.t("common.listSeparator");
  let (task, key) = (detail.task.id(), detail.task.key().to_string());
  let (number, branch) = detail.run.as_ref().map_or_else(Default::default, |run| {
    let branch = run.run.base_branch().map_or_else(String::new, ToString::to_string);
    (run.run.number().get(), branch)
  });
  let args = MergeArgs {
    open: open.clone(),
    task,
    key,
    number,
    branch,
    run: detail.run.as_ref().expect("Ready for review has a run").id(),
    programs,
    activity,
    separator,
    changed: Arc::new(move || nav.tasks_changed()),
  };
  let run_job = job.clone();
  MergeAction {
    job,
    start: Arc::new(move || run_job.start_merge(args.clone())),
  }
}

/// The readiness card (`IBuWf`): ready for review, or where the run's work was merged.
pub(super) fn readiness(ctx: &Ctx, merge: Option<&RunMerge>, leftovers: Option<&Leftovers>) -> Element {
  let (glyph, title, detail) = match merge {
    None if leftovers.is_some_and(|leftovers| !leftovers.keeps_run_worktree()) => (
      Icon::FolderX,
      ctx.t("runs.ready.title"),
      ctx.t("runs.ready.worktreeRemovedDetail"),
    ),
    None => (Icon::CircleCheck, ctx.t("runs.ready.title"), ctx.t("runs.ready.detail")),
    Some(merge) => (
      Icon::GitMerge,
      ctx.t_args("runs.merge.mergedTitle", [("branch", merge.into.as_str())]),
      merged_detail(ctx, merge),
    ),
  };
  let left = merge.and(leftovers).map(|leftovers| {
    Text::new(&left_behind(ctx, leftovers))
      .id("task-ready-left")
      .variant(TypographyStyle::FieldLabel)
  });
  Row::new()
    .id("task-ready-notice")
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(READINESS_HEIGHT))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Group.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(glyph, IconSize::Panel, PaletteColor::TextPrimary))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Indicator.role())
        .child(Text::new(&title).variant(ExtraTypography::CardTitle.role()))
        .child(Text::new(&detail).variant(TypographyStyle::FieldLabel))
        .with_children(left),
    )
    .into()
}

/// "main was fast-forwarded to 1a2b3c4 at 14:40."
fn merged_detail(ctx: &Ctx, merge: &RunMerge) -> Arc<str> {
  let key = match merge.kind {
    MergeKind::FastForward => "runs.merge.mergedDetail.fastForward",
    MergeKind::MergeCommit => "runs.merge.mergedDetail.mergeCommit",
    MergeKind::AlreadyMerged => "runs.merge.mergedDetail.alreadyMerged",
  };
  let time = display::clock(merge.merged_at);
  ctx.t_args(
    key,
    [
      ("branch", merge.into.as_str()),
      ("revision", display::short_revision(merge.revision.as_str())),
      ("time", time.as_str()),
    ],
  )
}

/// What is left of the run after its merge: nothing, worktrees kept for their uncommitted
/// changes, or branches that are not merged.
fn left_behind(ctx: &Ctx, leftovers: &Leftovers) -> Arc<str> {
  let kept_branches: Vec<&str> = leftovers
    .branches
    .iter()
    .filter(|branch| !branch.merged)
    .map(|branch| branch.name.as_str())
    .collect();
  let worktrees = leftovers.worktrees.len();
  match (worktrees, kept_branches.as_slice()) {
    (0, []) => ctx.t("runs.merge.left.nothing"),
    (0, branches) => ctx.t_args(
      "runs.merge.left.branches",
      [("branches", branches.join(&ctx.t("common.listSeparator")).as_str())],
    ),
    (1, _) => ctx.t("runs.merge.left.worktree.one"),
    (count, _) => ctx.t_args(
      "runs.merge.left.worktree.other",
      [("count", count.to_string().as_str())],
    ),
  }
}

/// *Merge into main* (`git-merge`), "Merging…" while it runs; unavailable when the run did not
/// record its base branch.
pub(super) fn merge_button(
  ctx: &Ctx,
  detail: &TaskDetail,
  action: &MergeAction,
  button_ref: &ElementRef,
) -> Option<Element> {
  if detail.merge.is_some() {
    return None;
  }
  let run = detail.run.as_ref()?;
  let state = action.job.merge().get();
  let pending = matches!(state, JobState::Running);
  let removal_running = matches!(action.job.remove().get(), JobState::Running);
  let busy = action.job.busy() || removal_running || state == JobState::Succeeded;
  let button = match run.run.base_branch() {
    None => ActionButton::new(ctx.t("runs.merge.mergeUnknown")).icon(Icon::GitMerge),
    Some(_) if pending => ActionButton::new(ctx.t("runs.merge.merging"))
      .icon(Icon::Loader)
      .width(Dimension::Px(action.job.merge_width().get()))
      .disabled(),
    Some(branch) => {
      let start = action.start.clone();
      ActionButton::new(ctx.t_args("runs.merge.merge", [("branch", branch.as_str())]))
        .icon(Icon::GitMerge)
        .on_click(move || start())
        .enabled(!busy)
    }
  };
  Some(
    button
      .id("task-merge")
      .ref_element(button_ref.clone())
      .min_width(Dimension::Px(MERGE_BUTTON_WIDTH))
      .render(ctx),
  )
}

/// Why the last merge did not happen, above the actions; nothing while one runs or after one
/// succeeded.
pub(super) fn merge_problem(ctx: &Ctx, detail: &TaskDetail, action: &MergeAction) -> Option<Element> {
  let JobState::Failed(error) = action.job.merge().get() else {
    return None;
  };
  if detail.merge.is_some() {
    return None;
  }
  let branch = detail.run.as_ref()?.run.base_branch()?.to_string();
  Some(
    Notice::new(
      Icon::AlertCircle,
      ctx.t_args("runs.merge.failure.title", [("branch", branch.as_str())]),
      message_text(ctx, &error),
    )
    .emphasized()
    .id("task-merge-problem")
    .into(),
  )
}

/// Why *Merge* is unavailable, under the actions; `None` when it is available or done.
pub(super) fn merge_unavailable(ctx: &Ctx, detail: &TaskDetail) -> Option<Arc<str>> {
  let run = detail.run.as_ref()?;
  (detail.merge.is_none() && run.run.base_branch().is_none()).then(|| ctx.t("runs.merge.unavailable"))
}
