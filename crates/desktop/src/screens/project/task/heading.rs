//! The task page heading (`DAsh8`): the task's title and where its run stands, e.g. "Run 128 ·
//! Attempt 1 · Implement · Builder working", and the words and icons for step states. A run ready
//! for review shows its status label and when it finished instead (`ruBkY` in 02.13 `vpgrJ`).

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::{PauseReason, RunFailure, RunLimit, RunStatus, StepStatus, Stuck, TaskPhase};

use super::{
  super::page,
  approval_card::waiting_request,
  run_approvals::{AutoApprovingLine, AutoApprovingProps},
};
use crate::{
  display,
  tasks::{
    approvals::ShownApproval,
    detail::{RunView, StepView, TaskDetail},
    unreadable::Unreadable,
  },
  theme::{ExtraSpacing, ExtraTypography},
  ui::{icon::Icon, status_label::status_label},
};

/// The heading, then a notice for each earlier run that cannot be read.
pub(super) fn with_notices(ctx: &mut Ctx, detail: &TaskDetail, lost: bool, repo_root: &Path) -> Vec<Element> {
  let heading = heading(ctx, detail, lost, repo_root);
  std::iter::once(heading)
    .chain(
      detail
        .earlier_unreadable
        .iter()
        .map(|run| super::unreadable::earlier_notice(ctx, run)),
    )
    .collect()
}

/// `lost`: the run is recorded as running but no runner is connected, so it is interrupted. While the
/// run auto-approves, *Auto-approving* and *Stop* follow (`run_approvals`).
fn heading(ctx: &mut Ctx, detail: &TaskDetail, lost: bool, repo_root: &Path) -> Element {
  let title = detail.task.details().title.as_str();
  if let Some(unreadable) = &detail.latest_unreadable {
    return page::heading(title, &unreadable_description(ctx, unreadable));
  }
  let waiting = detail
    .run
    .as_ref()
    .filter(|_| !lost)
    .and_then(|run| waiting_request(ctx, run).map(|shown| (run, shown)));
  let heading = match (&detail.run, waiting) {
    (_, Some((run, shown))) => waiting_heading(ctx, title, run, &shown),
    (Some(run), None) if *run.run.status() == RunStatus::Succeeded => {
      ready_heading(ctx, title, run, detail.merge.is_some())
    }
    _ => page::heading(title, &description(ctx, detail, lost)).id("task-heading"),
  };
  let auto = detail
    .run
    .as_ref()
    .filter(|run| !run.run.status().is_terminal() && run.approvals.auto_approving());
  let Some(run) = auto else {
    return heading;
  };
  let line = ctx.mount_keyed::<AutoApprovingLine>(
    &format!("task-auto-approving-{}", run.id()),
    AutoApprovingProps {
      run: run.id(),
      repo_root: repo_root.to_path_buf(),
    },
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(heading)
    .child(line)
    .into()
}

/// The title over "Needs you" and "Run 146 · Design review v2 · Design · Designer waits for your answer,
/// 14:06" while the run's agent waits for a tool decision (`wSMj8` in `G0rzJy`).
fn waiting_heading(ctx: &Ctx, title: &str, run: &RunView, shown: &ShownApproval) -> Element {
  let step = run.step_of(shown.approval.execution).map_or("", |step| &*step.name);
  let meta = ctx.t_args(
    "runs.page.describe.waitsForAnswer",
    [
      ("run", run.run.number().get().to_string().as_str()),
      ("pipeline", &*run.pipeline),
      ("version", run.version.to_string().as_str()),
      ("step", step),
      ("agent", shown.approval.agent.as_str()),
      ("time", display::clock(shown.approval.requested_at).as_str()),
    ],
  );
  status_heading(
    title,
    (Icon::Hand, &ctx.t("runs.page.needsYou")),
    &meta,
    "task-waiting-meta",
  )
}

/// The title over "Ready for review" and "Run #128 · Feature delivery · Finished at 14:32" (`ruBkY`);
/// "Merged" once the run's work was merged (not drawn yet).
fn ready_heading(ctx: &Ctx, title: &str, run: &RunView, merged: bool) -> Element {
  let (glyph, status) = if merged {
    (Icon::GitMerge, ctx.t("runs.merge.status"))
  } else {
    (Icon::GitPullRequest, ctx.t("runs.ready.status"))
  };
  let number = run.run.number().get().to_string();
  let finished = run.finished_at.map(display::clock).unwrap_or_default();
  let meta = ctx.t_args(
    "runs.ready.meta",
    [
      ("run", number.as_str()),
      ("pipeline", &*run.pipeline),
      ("time", finished.as_str()),
    ],
  );
  status_heading(title, (glyph, &status), &meta, "task-ready-meta")
}

/// The title over a status label and the run's line (`ruBkY`, `wSMj8`).
fn status_heading(title: &str, (glyph, status): (Icon, &str), meta: &str, meta_id: &'static str) -> Element {
  Column::new()
    .id("task-heading")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Rail.role())
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(ExtraSpacing::HeadingBottom.role())
    .spacing(ExtraSpacing::Group.role())
    .child(Text::new(title).variant(ExtraTypography::PageTitle.role()))
    .child(
      Row::new()
        .spacing(ExtraSpacing::Group.role())
        .align_items(Alignment::Center)
        .child(status_label(glyph, status))
        .child(Text::new(meta).id(meta_id).variant(TypographyStyle::Caption).nowrap()),
    )
    .into()
}

/// "Run 131 · could not be read" (`PrBzq` in 02.R).
fn unreadable_description(ctx: &Ctx, run: &Unreadable) -> Arc<str> {
  match run.number {
    Some(number) => ctx.t_args(
      "runs.page.describe.unreadable",
      [("run", number.get().to_string().as_str())],
    ),
    None => ctx.t("runs.page.describe.unreadableUnnumbered"),
  }
}

fn description(ctx: &Ctx, detail: &TaskDetail, lost: bool) -> Arc<str> {
  let Some(run) = &detail.run else {
    return match detail.phase() {
      TaskPhase::Draft => ctx.t("runs.page.draft"),
      _ => ctx.t("runs.page.queued"),
    };
  };
  let number = run.run.number().get().to_string();
  let attempt = run
    .run
    .current_attempt()
    .map_or_else(String::new, |attempt| attempt.number().to_string());
  let step = run.current_step();
  let working: Vec<&str> = run.working_steps().map(|step| &*step.name).collect();
  let parallel = working.join(&ctx.t("runs.progress.together"));
  let step_name = step.map_or("", |step| &step.name);
  let agent = step.and_then(|step| step.agent.as_deref()).unwrap_or_default();
  let time = run.paused_at.map(display::clock).unwrap_or_default();
  let version = run.version.to_string();
  let args = [
    ("run", number.as_str()),
    ("attempt", attempt.as_str()),
    ("step", step_name),
    ("agent", agent),
    ("time", time.as_str()),
    ("steps", parallel.as_str()),
    ("pipeline", &run.pipeline),
    ("version", version.as_str()),
  ];
  let awaiting_plan = run.run.waits_for_a_person() && !run.run.awaiting_plan().is_empty();
  let asking = run.run.waits_for_a_person() && !run.run.awaiting_answer().is_empty();
  let key = match (run.run.status(), step) {
    (RunStatus::Paused(PauseReason::LimitReached(RunLimit::GroupBudget)), _) => "tasks.bulk.stoppedGroup",
    (RunStatus::Running, _) if lost => "runs.page.describe.interrupted",
    (RunStatus::Running, _) if awaiting_plan => "runs.page.describe.awaitingPlan",
    (RunStatus::Running, _) if asking => "runs.page.describe.asking",
    (RunStatus::Pending, _) if run.group_hold == Some(orchester_core::task_group::GroupHold::SessionEnded) => {
      "runs.page.describe.schedulingHeld"
    }
    (RunStatus::Pending, _) => "runs.page.describe.pending",
    (RunStatus::Running, _) if run.run.pause_requested() => "runs.page.describe.pausing",
    (RunStatus::Running, Some(_)) if working.len() > 1 => "runs.page.describe.workingParallel",
    (RunStatus::Running, Some(_)) => "runs.page.describe.working",
    (RunStatus::Running, None) => "runs.page.describe.starting",
    (RunStatus::Paused(PauseReason::Requested), _) => "runs.page.describe.paused",
    (RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens)), _) => "runs.page.describe.tokenLimit",
    (RunStatus::Paused(PauseReason::LimitReached(RunLimit::Time)), _) => "runs.page.describe.timeLimit",
    (RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget)), _) => "runs.page.describe.dailyBudget",
    (RunStatus::Paused(PauseReason::LimitReached(RunLimit::StepBudget { .. })), _) => "runs.page.describe.stepBudget",
    (RunStatus::Interrupted(_), _) => "runs.page.describe.interrupted",
    (RunStatus::Succeeded, _) => "runs.page.describe.succeeded",
    (
      RunStatus::Failed(RunFailure::RetriesExhausted(_)) | RunStatus::WaitingForPerson(Stuck::RetriesExhausted(_)),
      _,
    ) => "runs.page.describe.retriesExhausted",
    (RunStatus::WaitingForPerson(Stuck::BrokeAgain(_)), _) => "runs.page.describe.brokeAgain",
    (RunStatus::Failed(_), _) => "runs.page.describe.failed",
    (RunStatus::Cancelled, _) if run.run.current_attempt().is_none() => "runs.page.describe.cancelledBeforeStart",
    (RunStatus::Cancelled, _) => "runs.page.describe.cancelled",
  };
  ctx.t_args(key, args)
}

/// Icon and word of a step's state (`DeizF`); a step that has not started waits.
pub(super) fn step_state(status: Option<StepStatus>) -> (Icon, &'static str) {
  match status {
    None | Some(StepStatus::Pending) => (Icon::Clock3, "runs.step.pending"),
    Some(StepStatus::Running) => (Icon::Loader, "runs.step.running"),
    Some(StepStatus::Waiting(_)) => (Icon::Clock3, "runs.step.waiting"),
    Some(StepStatus::Passed) => (Icon::Check, "runs.step.passed"),
    Some(StepStatus::Failed) => (Icon::AlertCircle, "runs.step.failed"),
    Some(StepStatus::Skipped) => (Icon::CircleMinus, "runs.step.skipped"),
  }
}

/// "Builder" or, for a step without an agent, nothing.
pub(super) fn agent_of(step: &StepView) -> &str {
  step.agent.as_deref().unwrap_or_default()
}
