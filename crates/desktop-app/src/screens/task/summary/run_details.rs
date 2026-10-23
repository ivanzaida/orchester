//! *Run details* beside the Summary of a run that ended (`Run details` in `F2LtFn`): the project, where
//! the run started and worked, its last checkpoint, how long it took from its start to its end, the
//! tokens it used against its budget, its cached input, the agents of its steps, and what the last
//! agent step's agent ran with. A run that is still under way shows *This run* instead.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{run::StepStatus, tokens::compact_tokens};
use orchester_workspace::{display, tasks::detail::RunView};

use super::this_run::{base_value, cached_row, checkpoint_row, panel, permissions, row};
use crate::{screens::task::TaskContext, ui::detail_row::DetailValue};

const MILLIS_PER_MINUTE: i64 = 60_000;

/// *Run details* of `run`, `lead` above it (returning its work to a step), the task's own `section` above its title.
pub fn run_details(
  ctx: &mut Ctx,
  task: &TaskContext,
  run: &RunView,
  (lead, section): (Option<Element>, Option<Element>),
) -> Element {
  let project = task.project.open.project.name().as_str().to_owned();
  let mut rows = vec![
    row(ctx, "summary.runDetails.project", &project, DetailValue::Text),
    row(ctx, "summary.runDetails.base", &base_value(ctx, run), DetailValue::Mono),
  ];
  // A run that never started has no branch of its own.
  rows.extend(
    run
      .run
      .own_branch()
      .map(|branch| row(ctx, "summary.runDetails.branch", branch.as_str(), DetailValue::Mono)),
  );
  rows.push(checkpoint_row(ctx, run));
  rows.extend(duration(ctx, run).map(|value| row(ctx, "summary.runDetails.duration", &value, DetailValue::Text)));
  rows.push(row(
    ctx,
    "summary.runDetails.tokens",
    &tokens(ctx, run),
    DetailValue::Text,
  ));
  rows.push(cached_row(ctx, run));
  let agents = agents(run);
  if !agents.is_empty() {
    let list = agents.join(&ctx.t("task.listSeparator"));
    rows.push(row(ctx, "summary.runDetails.agents", &list, DetailValue::Text));
  }
  rows.extend(permissions(ctx, run));
  rows.extend(super::decisions::delivery_rows(ctx, task.detail.queue.as_ref()));
  panel(
    "task-run-details",
    &ctx.t("summary.runDetails.title"),
    rows,
    (lead, section),
  )
}

/// "38 min · 16:14 – 16:52": from its first agent's start to its end.
fn duration(ctx: &Ctx, run: &RunView) -> Option<Arc<str>> {
  let (start, end) = (run.invocations.first()?.started_at, run.finished_at?);
  let minutes = (end.unix_millis() - start.unix_millis()).max(0) / MILLIS_PER_MINUTE;
  Some(ctx.t_args(
    "summary.runDetails.durationValue",
    [
      ("minutes", minutes.to_string()),
      ("start", display::clock(start)),
      ("end", display::clock(end)),
    ],
  ))
}

/// "612k · no limit", or "612k of 1.4M".
fn tokens(ctx: &Ctx, run: &RunView) -> Arc<str> {
  let used = compact_tokens(run.run.used().get());
  let Some(budget) = run.run.budget() else {
    return ctx.t_args("summary.runDetails.tokensNoLimit", [("used", used)]);
  };
  ctx.t_args(
    "summary.runDetails.tokensValue",
    [("used", used), ("budget", compact_tokens(budget.get()))],
  )
}

/// The agents of the steps that ran, in pipeline order, each once.
fn agents(run: &RunView) -> Vec<String> {
  let mut agents: Vec<String> = Vec::new();
  let ran = run.steps.iter().filter(|step| {
    !step.tool_step
      && matches!(
        step.status,
        Some(StepStatus::Running | StepStatus::Waiting(_) | StepStatus::Passed | StepStatus::Failed)
      )
  });
  for agent in ran.filter_map(|step| step.agent.as_deref()) {
    if !agents.iter().any(|known| known == agent) {
      agents.push(agent.to_owned());
    }
  }
  agents
}
