//! The task page heading (`DAsh8`): the task's title and where its run stands, e.g. "Run 128 ·
//! Attempt 1 · Implement · Builder working", and the words and icons for step states.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::run::{RunStatus, StepStatus, TaskPhase};

use super::super::page;
use crate::{
  tasks::detail::{StepView, TaskDetail},
  ui::icon::Icon,
};

pub(super) fn heading(ctx: &Ctx, detail: &TaskDetail) -> Element {
  page::heading(detail.task.details().title.as_str(), &description(ctx, detail))
}

fn description(ctx: &Ctx, detail: &TaskDetail) -> Arc<str> {
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
  let step_name = step.map_or("", |step| &step.name);
  let agent = step.and_then(|step| step.agent.as_deref()).unwrap_or_default();
  let args = [
    ("run", number.as_str()),
    ("attempt", attempt.as_str()),
    ("step", step_name),
    ("agent", agent),
  ];
  let key = match (run.run.status(), step) {
    (RunStatus::Pending, _) => "runs.page.describe.pending",
    (RunStatus::Running, Some(_)) => "runs.page.describe.working",
    (RunStatus::Running, None) => "runs.page.describe.starting",
    (RunStatus::Paused, _) => "runs.page.describe.paused",
    (RunStatus::Interrupted(_), _) => "runs.page.describe.interrupted",
    (RunStatus::Succeeded, _) => "runs.page.describe.succeeded",
    (RunStatus::Failed(_), _) => "runs.page.describe.failed",
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
