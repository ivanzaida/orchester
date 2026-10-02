use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{connection::AgentProvider, time::Timestamp, tokens::grouped};

use crate::{
  display::{self, WEEKDAY_KEYS},
  screens::project::connections::status::short_key,
};

const DAY_MINUTES: u64 = 24 * 60;

/// The CLI's short name, as the status bar item shows it ("Codex", "Claude").
pub fn name(ctx: &Ctx, provider: AgentProvider) -> Arc<str> {
  ctx.t(short_key(provider))
}

/// The CLI's name in a hint heading, where Claude Code is named in full.
pub fn hint_name(ctx: &Ctx, provider: AgentProvider) -> Arc<str> {
  match provider {
    AgentProvider::ClaudeCodeCli => ctx.t("statusBar.planLimits.claudeFull"),
    provider => name(ctx, provider),
  }
}

pub fn plan(ctx: &Ctx, plan: &str) -> Arc<str> {
  match plan {
    "pro" => ctx.t("statusBar.planLimits.planPro"),
    "plus" => ctx.t("statusBar.planLimits.planPlus"),
    plan => ctx.t_args("statusBar.planLimits.planOther", [("plan", plan)]),
  }
}

/// A used percentage in whole percent, as every limit figure reads.
pub fn percent(ctx: &Ctx, percent: f64) -> Arc<str> {
  let whole = percent.round() as u64;
  ctx.t_args("statusBar.planLimits.percent", [("used", whole.to_string().as_str())])
}

/// A credit balance as reported, its digits grouped as the locale writes numbers when it is a
/// whole number (`1,250`); anything else is shown as the CLI wrote it.
pub fn balance(ctx: &Ctx, balance: &str) -> String {
  balance.parse::<u64>().map_or_else(
    |_| balance.to_owned(),
    |whole| grouped(whole, &ctx.t("number.groupSeparator")),
  )
}

/// The local clock time of `at` (`16:40`).
pub fn clock(at: Timestamp) -> String {
  display::clock(at)
}

pub fn reset(ctx: &Ctx, at: Option<Timestamp>, now: Timestamp, relative: bool) -> Arc<str> {
  let Some(at) = at else {
    return ctx.t("statusBar.planLimits.resetUnknown");
  };
  let minutes = (at.unix_millis().saturating_sub(now.unix_millis())).max(0) as u64;
  let minutes = minutes.div_ceil(60_000);
  if relative && minutes < 60 {
    return ctx.t_args(
      "statusBar.planLimits.resetsInMinutes",
      [("minutes", minutes.to_string().as_str())],
    );
  }
  if relative && minutes < DAY_MINUTES {
    return ctx.t_args(
      "statusBar.planLimits.resetsIn",
      [
        ("hours", (minutes / 60).to_string().as_str()),
        ("minutes", (minutes % 60).to_string().as_str()),
      ],
    );
  }
  if minutes < DAY_MINUTES {
    return ctx.t_args("statusBar.planLimits.resetsAt", [("time", clock(at).as_str())]);
  }
  let weekday = ctx.t(WEEKDAY_KEYS[display::weekday(at)]);
  ctx.t_args(
    "statusBar.planLimits.resetsOnDay",
    [("weekday", &*weekday), ("time", clock(at).as_str())],
  )
}

/// A window's name by its length: 5-hour and Weekly as the CLIs call them, else whole days,
/// hours or minutes.
pub fn window(ctx: &Ctx, minutes: u64) -> Arc<str> {
  let (key, count) = match minutes {
    300 => return ctx.t("statusBar.planLimits.fiveHour"),
    10080 => return ctx.t("statusBar.planLimits.weekly"),
    minutes if minutes % DAY_MINUTES == 0 => ("statusBar.planLimits.days", minutes / DAY_MINUTES),
    minutes if minutes % 60 == 0 => ("statusBar.planLimits.hours", minutes / 60),
    minutes => ("statusBar.planLimits.minutes", minutes),
  };
  ctx.t_args(key, [("count", count.to_string().as_str())])
}
