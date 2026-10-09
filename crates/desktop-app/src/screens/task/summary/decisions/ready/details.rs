//! *Run details* beside Ready for review (`This run` `Q2TFNN` in `F2LtFn`): the project, the revision and
//! branch the run started from and worked on, its last checkpoint, how long it took and when, its tokens
//! and cached input, its agents, and what the last agent step's agent could do. The run's approvals come
//! later with *This run*'s (agreed deviation D4).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{time::Timestamp, tokens::compact_tokens};
use orchester_workspace::{budget::unbounded, display, tasks::detail::RunView};

use super::super::{TaskContext, this_run};
use crate::ui::{
  detail_row::{DetailValue, detail_row},
  message::message_text,
};

pub fn run_details(ctx: &mut Ctx, task: &TaskContext, run: &RunView, now: Timestamp) -> Element {
  let row = |ctx: &Ctx, (id, key): (&str, &str), value: &str, kind: DetailValue| -> Element {
    detail_row(&format!("task-run-details-{id}"), &ctx.t(key), value, kind).into()
  };
  let base = match run.run.base_branch() {
    Some(branch) => ctx.t_args(
      "decisions.details.baseValue",
      [
        ("branch", branch.as_str()),
        ("revision", display::short_revision(run.run.base().as_str())),
      ],
    ),
    None => Arc::from(display::short_revision(run.run.base().as_str())),
  };
  let mut rows = vec![
    row(
      ctx,
      ("project", "decisions.details.project"),
      task.project.open.project.name().as_str(),
      DetailValue::Text,
    ),
    row(ctx, ("base", "decisions.details.base"), &base, DetailValue::Mono),
    row(
      ctx,
      ("branch", "decisions.details.branch"),
      run.run.branch().as_str(),
      DetailValue::Mono,
    ),
  ];
  if let Some(checkpoint) = run.run.last_checkpoint() {
    let value = ctx.t_args(
      "decisions.details.checkpointValue",
      [
        (
          "revision",
          display::short_revision(checkpoint.revision().as_str()).to_owned(),
        ),
        ("time", display::clock(checkpoint.created_at())),
      ],
    );
    rows.push(row(
      ctx,
      ("checkpoint", "decisions.details.checkpoint"),
      &value,
      DetailValue::Mono,
    ));
  }
  rows.extend(duration(ctx, run, now).map(|value| {
    row(
      ctx,
      ("duration", "decisions.details.duration"),
      &value,
      DetailValue::Text,
    )
  }));
  rows.push(row(
    ctx,
    ("tokens", "decisions.details.tokens"),
    &tokens(ctx, run),
    DetailValue::Text,
  ));
  let cached = ctx.t_args(
    "decisions.details.cachedValue",
    [("count", compact_tokens(run.usage().cached_input))],
  );
  rows.push(row(
    ctx,
    ("cached", "decisions.details.cached"),
    &cached,
    DetailValue::Text,
  ));
  rows.extend(
    agents(ctx, run).map(|agents| row(ctx, ("agents", "decisions.details.agents"), &agents, DetailValue::Text)),
  );
  rows.extend(this_run::permissions(ctx, run));
  this_run::panel("task-run-details", &ctx.t("decisions.details.title"), rows, None)
}

/// "38 min · 16:14 – 16:52": from the run's first attempt to its end.
fn duration(ctx: &Ctx, run: &RunView, now: Timestamp) -> Option<Arc<str>> {
  let start = run.attempt_starts.first()?.at;
  let end = run.finished_at.unwrap_or(now);
  // A clock set back since the run started reads as no time.
  let took = std::time::Duration::from_millis(u64::try_from(end.unix_millis() - start.unix_millis()).unwrap_or(0));
  Some(ctx.t_args(
    "decisions.details.durationValue",
    [
      ("duration", message_text(ctx, &display::duration(took)).to_string()),
      ("start", display::clock(start)),
      ("end", display::clock(end)),
    ],
  ))
}

/// "612k · no limit" or "612k of 1.4M".
fn tokens(ctx: &Ctx, run: &RunView) -> Arc<str> {
  let used = compact_tokens(run.run.used().get());
  if run.run.budget() == unbounded() {
    return ctx.t_args("decisions.details.tokensNoLimit", [("used", used)]);
  }
  ctx.t_args(
    "decisions.details.tokensValue",
    [("used", used), ("budget", compact_tokens(run.run.budget().get()))],
  )
}

/// The agents of the run's agent steps that ran, each once, in pipeline order.
fn agents(ctx: &Ctx, run: &RunView) -> Option<Arc<str>> {
  let mut names: Vec<&str> = Vec::new();
  for agent in run
    .steps
    .iter()
    .filter(|step| step.execution.is_some() && !step.tool_step)
    .filter_map(|step| step.agent.as_deref())
  {
    if !names.contains(&agent) {
      names.push(agent);
    }
  }
  (!names.is_empty()).then(|| Arc::from(names.join(&ctx.t("task.listSeparator"))))
}
