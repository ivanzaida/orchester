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
use orchester_core::run::{PauseReason, RunStatus, TaskPhase};
use orchester_workspace::{
  display,
  tasks::detail::{RunView, StepView, TaskDetail},
};

use crate::{
  theme::{ControlHeight, ExtraTypography},
  ui::status::{Status, StatusChip},
};

pub fn status_row(ctx: &mut Ctx, detail: &TaskDetail, lost: bool) -> Element {
  let (status, step) = status(detail, lost);
  let mut chip = StatusChip::new("task-status", status);
  if let Some(step) = step {
    chip = chip.detail(step_label(ctx, step));
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
      | TaskPhase::Stuck
      | TaskPhase::Paused
      | TaskPhase::StoppedAtLimit
      | TaskPhase::Interrupted
      | TaskPhase::Failed
  );
  (Status::of_phase(phase), step.filter(|_| with_step))
}

/// "03 Implement".
fn step_label(ctx: &Ctx, step: &StepView) -> Arc<str> {
  ctx.t_args(
    "task.status.step",
    [("number", format!("{:02}", step.number)), ("step", step.name.to_string())],
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
