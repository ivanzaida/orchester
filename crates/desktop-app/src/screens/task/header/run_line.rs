//! The row under the title (`Organization` `foLAM` in `lUXff`): where the task's run stands as a status
//! (`Status · Running · 03 Implement` `tjx4l`), and at its end the run line (`Run line` `vKpQg`): "Run
//! 128 · attempt 2 · Feature delivery v3 · started 16:20". The organization pickers that the design
//! puts before the status come later (agreed deviation D4).

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  integration::MergeError,
  run::{PauseReason, Run, RunLimit, RunStatus, Stuck, TaskPhase},
};
use orchester_workspace::{
  display,
  tasks::{
    delivery::Delivery,
    detail::{RunView, StepView, TaskDetail},
    integration::IntegrationProblem,
    integration_jobs::JobState,
  },
};

use crate::{
  screens::task::TaskContext,
  theme::{ControlHeight, ExtraTypography},
  ui::status::{Status, StatusChip},
};

pub fn status_row(ctx: &mut Ctx, task: &TaskContext) -> Element {
  let detail = &task.detail;
  let (status, detail_words) = chip_words(ctx, task);
  let mut chip = StatusChip::new("task-status", status);
  if let Some(words) = detail_words {
    chip = chip.detail(words);
  }
  Row::new()
    .id("task-status-row")
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ControlHeight::Md.value()))
    .align_items(Alignment::Center)
    .child(chip.render(ctx))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&run_line(ctx, detail))
        .id("task-run-line")
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .into()
}

/// The task's status and what it is about: the step it stands at, why the run waits for the person's decision
/// ([`stuck_words`]), or why a merge failed ("conflict").
fn chip_words(ctx: &Ctx, task: &TaskContext) -> (Status, Option<Arc<str>>) {
  let detail = &task.detail;
  let lost = task.lost();
  if !lost && detail.phase() == TaskPhase::Stuck {
    let words = detail.run.as_ref().and_then(|run| stuck_words(&run.run));
    return (Status::NeedsInput, words.map(|key| ctx.t(key)));
  }
  // A request the run's agent waits on: the run needs the person, as its notice and steps say.
  if let Some(approval) = task.waiting_approval() {
    let step = detail
      .run
      .as_ref()
      .and_then(|run| run.step_of(approval.approval.execution));
    return (Status::NeedsInput, step.map(|step| step_label(ctx, step)));
  }
  if let Some(conflict) = merge_failed(task) {
    let words = conflict.then(|| ctx.t("task.status.conflict"));
    return (Status::NotMerged, words);
  }
  let (status, step) = status(detail, lost);
  (status, step.map(|step| step_label(ctx, step)))
}

/// Why a run waits for the person's decision: its step used up its retries or broke again ("retries used", as the
/// design's chip says), reported the work failed in a follow-up the pipeline has no route for, could not check
/// everything, or was not run again on an unchanged revision.
fn stuck_words(run: &Run) -> Option<&'static str> {
  match run.status() {
    RunStatus::WaitingForPerson(_) if run.waits_without_route() => Some("task.status.failed"),
    RunStatus::WaitingForPerson(Stuck::RetriesExhausted(_) | Stuck::BrokeAgain(_)) => Some("task.status.retriesUsed"),
    RunStatus::WaitingForPerson(Stuck::Unverified(_)) => Some("task.status.unverified"),
    RunStatus::WaitingForPerson(Stuck::Unchanged(_)) => Some("task.status.unchanged"),
    _ => None,
  }
}

/// Whether merging the latest run failed in this session, and whether it failed on a conflict. The
/// merge's own panel says why in full.
fn merge_failed(task: &TaskContext) -> Option<bool> {
  // A run whose work went back to a step after its merge failed works again: the failure is behind it.
  let run = task
    .detail
    .run
    .as_ref()
    .filter(|run| task.detail.merge.is_none() && *run.run.status() == RunStatus::Succeeded)?;
  let JobState::Failed(reason) = task.jobs.run(run.id()).merge else {
    return None;
  };
  let conflict = IntegrationProblem::Merge(MergeError::Conflict { files: Vec::new() }).message("");
  Some(reason.key == conflict.key)
}

/// The task's status, and the step it stands at while that says more.
fn status(detail: &TaskDetail, lost: bool) -> (Status, Option<&StepView>) {
  if detail.latest_unreadable.is_some() {
    return (Status::Unreadable, None);
  }
  let step = detail.run.as_ref().and_then(RunView::current_step);
  if lost {
    return (Status::Interrupted, step);
  }
  let phase = detail.phase();
  let with_step = matches!(
    phase,
    TaskPhase::Running
      | TaskPhase::Waiting
      | TaskPhase::Paused
      | TaskPhase::StoppedAtLimit
      | TaskPhase::Interrupted
      | TaskPhase::Failed
  );
  // A step whose agent never started waits for its account's reset (`XPjSc`); one that started stopped at the limit.
  let plan_limit = detail.run.as_ref().and_then(|run| match run.run.status() {
    RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { execution })) => {
      Some(run.invocations_of(*execution).next().is_some())
    }
    _ => None,
  });
  let status = match plan_limit {
    Some(true) => Status::StoppedAtPlanLimit,
    Some(false) => Status::WaitingForPlanReset,
    // A run that committed nothing completed: it waits for no review of work to merge.
    None if detail.delivery == Delivery::NothingToMerge => Status::of_phase(phase),
    None => Status::of_run(phase),
  };
  (status, step.filter(|_| with_step))
}

/// "03 Implement".
fn step_label(ctx: &Ctx, step: &StepView) -> Arc<str> {
  ctx.t_args(
    "task.status.step",
    [
      ("number", format!("{:02}", step.number)),
      ("step", step.name.to_string()),
    ],
  )
}

fn run_line(ctx: &Ctx, detail: &TaskDetail) -> Arc<str> {
  if let Some(unreadable) = &detail.latest_unreadable {
    return match unreadable.number {
      Some(number) => ctx.t_args("task.runLine.unreadable", [("run", number.get().to_string())]),
      None => ctx.t("task.runLine.unreadableUnnumbered"),
    };
  }
  let Some(run) = &detail.run else {
    return match detail.phase() {
      TaskPhase::Draft => ctx.t("task.runLine.draft"),
      _ => ctx.t("task.runLine.queued"),
    };
  };
  let attempt = run
    .run
    .current_attempt()
    .map(|attempt| attempt.number().get())
    .filter(|number| *number > 1);
  let args = [
    ("run", run.run.number().get().to_string()),
    ("attempt", attempt.map(|number| number.to_string()).unwrap_or_default()),
    ("pipeline", run.pipeline.to_string()),
    ("version", run.version.to_string()),
    ("when", when(ctx, run).map(|when| when.to_string()).unwrap_or_default()),
  ];
  let key = match (attempt.is_some(), when(ctx, run).is_some()) {
    (true, true) => "task.runLine.attemptWhen",
    (true, false) => "task.runLine.attempt",
    (false, true) => "task.runLine.when",
    (false, false) => "task.runLine.run",
  };
  ctx.t_args(key, args)
}

/// When the run last changed where it stands: it finished, paused or stopped, or else started.
fn when(ctx: &Ctx, run: &RunView) -> Option<Arc<str>> {
  let (key, at) = if let Some(at) = run.finished_at {
    ("task.runLine.finished", at)
  } else if let Some(at) = run.paused_at {
    let key = match run.run.status() {
      RunStatus::Paused(PauseReason::Requested) => "task.runLine.paused",
      _ => "task.runLine.stopped",
    };
    (key, at)
  } else {
    ("task.runLine.started", run.invocations.first()?.started_at)
  };
  Some(ctx.t_args(key, [("time", display::clock(at))]))
}
