//! How the Modules screens word a module: its dates, its state, its progress and its forecast (ORC-50) in a list row
//! (`Likely Oct 30`, *after target* in `warning`) and on the status line and the Summary's burndown line (`Likely done
//! by Oct 30 (50%) or Nov 2 (85%)`).

use std::sync::Arc;

use chrono::NaiveDate;
use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::module::{ModuleDetails, ModuleState};
use orchester_workspace::{
  delivery::{Forecast, Outlook},
  modules::{data::Progress, measure::WorkUnit},
};

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

/// What the chart counts in: *Counted in tasks*.
pub fn unit(ctx: &Ctx, unit: WorkUnit) -> Arc<str> {
  ctx.t(match unit {
    WorkUnit::Tasks => "modules.unit.tasks",
  })
}

/// A list row's forecast and its tone: `success` by the target date, `warning` after it, `text_secondary` without
/// one; *Not enough history* and *All done* muted.
pub fn forecast_row(ctx: &Ctx, forecast: &Forecast, target: Option<NaiveDate>) -> (Arc<str>, PaletteColor) {
  let Outlook::Dates { likely, .. } = &forecast.outlook else {
    return (forecast_line(ctx, forecast), PaletteColor::TextMuted);
  };
  let (key, tone) = match target {
    Some(target) if *likely > target => ("modules.forecast.afterTarget", PaletteColor::Warning),
    Some(_) => ("modules.forecast.likely", PaletteColor::Success),
    None => ("modules.forecast.likely", PaletteColor::TextSecondary),
  };
  (ctx.t_args(key, [("date", day(ctx, *likely))]), tone)
}

/// The forecast with both of its dates, or why it has none.
pub fn forecast_line(ctx: &Ctx, forecast: &Forecast) -> Arc<str> {
  match &forecast.outlook {
    Outlook::Done => ctx.t("modules.forecast.done"),
    Outlook::NotEnoughHistory => ctx.t("modules.forecast.notEnough"),
    Outlook::Dates { likely, safe, .. } => ctx.t_args(
      "modules.forecast.dates",
      [("likely", day(ctx, *likely)), ("safe", day(ctx, *safe))],
    ),
  }
}

/// The status line after the state: `Oct 14 – Nov 1 · 8 of 12 tasks done · Likely done by …`.
pub fn facts(ctx: &Ctx, details: &ModuleDetails, progress: Progress, forecast: &Forecast) -> Arc<str> {
  ctx.t_args(
    "modules.page.facts",
    [
      ("dates", dates(ctx, details)),
      ("progress", self::progress(ctx, progress)),
      ("forecast", forecast_line(ctx, forecast)),
    ],
  )
}
