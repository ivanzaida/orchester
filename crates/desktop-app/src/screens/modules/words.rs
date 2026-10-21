//! How the Modules screens word a module: its dates, its state and its progress.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::module::{ModuleDetails, ModuleState};
use orchester_workspace::modules::{data::Progress, measure::WorkUnit};

use crate::screens::usage::words::day;

/// `Oct 1 – Oct 15`, `From Oct 1`, `Until Oct 15`, or *No dates*.
pub fn dates(ctx: &Ctx, details: &ModuleDetails) -> Arc<str> {
  match (details.start(), details.target()) {
    (Some(start), Some(target)) => ctx.t_args(
      "modules.dates.range",
      [("start", &*day(ctx, start)), ("target", &*day(ctx, target))],
    ),
    (Some(start), None) => ctx.t_args("modules.dates.from", [("start", &*day(ctx, start))]),
    (None, Some(target)) => ctx.t_args("modules.dates.until", [("target", &*day(ctx, target))]),
    (None, None) => ctx.t("modules.dates.none"),
  }
}

pub fn state(ctx: &Ctx, state: ModuleState) -> Arc<str> {
  ctx.t(state_key(state))
}

pub const fn state_key(state: ModuleState) -> &'static str {
  match state {
    ModuleState::Open => "modules.state.open",
    ModuleState::Closed => "modules.state.closed",
  }
}

/// `3 of 8 tasks done`, or *Nothing in scope*.
pub fn progress(ctx: &Ctx, progress: Progress) -> Arc<str> {
  if progress.scope == 0 {
    return ctx.t("modules.progress.none");
  }
  let (done, scope) = (progress.done.to_string(), progress.scope.to_string());
  match progress.unit {
    WorkUnit::Tasks => ctx.t_args(
      "modules.progress.tasks",
      [("done", done.as_str()), ("scope", scope.as_str())],
    ),
  }
}

/// `38%`, or nothing for a module without work in scope.
pub fn percent(ctx: &Ctx, progress: Progress) -> Option<Arc<str>> {
  progress
    .percent()
    .map(|percent| ctx.t_args("modules.progress.percent", [("percent", percent.to_string().as_str())]))
}

/// What the chart counts in: *Counted in tasks*.
pub fn unit(ctx: &Ctx, unit: WorkUnit) -> Arc<str> {
  ctx.t(match unit {
    WorkUnit::Tasks => "modules.unit.tasks",
  })
}
