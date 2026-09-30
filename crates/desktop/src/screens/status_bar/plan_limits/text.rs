use std::sync::Arc;

use chrono::{DateTime, Local};
use lurq::app::ctx::Ctx;
use orchester_core::{connection::AgentProvider, time::Timestamp};

pub fn name(ctx: &Ctx, provider: AgentProvider) -> Arc<str> {
  ctx.t(match provider {
    AgentProvider::CodexCli => "statusBar.planLimits.codex",
    AgentProvider::ClaudeCodeCli => "statusBar.planLimits.claude",
  })
}

pub fn percent(ctx: &Ctx, percent: f64) -> Arc<str> {
  ctx.t_args(
    "statusBar.planLimits.percent",
    [("used", format!("{percent:.0}").as_str())],
  )
}

pub fn clock(at: Timestamp, day: bool) -> String {
  DateTime::from_timestamp_millis(at.unix_millis()).map_or_else(String::new, |at| {
    at.with_timezone(&Local)
      .format(if day { "%a %H:%M" } else { "%H:%M" })
      .to_string()
  })
}

pub fn reset(ctx: &Ctx, at: Option<Timestamp>, now: Timestamp, relative: bool) -> Arc<str> {
  let Some(at) = at else {
    return ctx.t("statusBar.planLimits.resetUnknown");
  };
  let minutes = (at.unix_millis().saturating_sub(now.unix_millis())).max(0) as u64;
  let minutes = minutes.div_ceil(60_000);
  if relative && minutes < 1440 {
    return ctx.t_args(
      "statusBar.planLimits.resetsIn",
      [
        ("hours", (minutes / 60).to_string().as_str()),
        ("minutes", (minutes % 60).to_string().as_str()),
      ],
    );
  }
  ctx.t_args(
    "statusBar.planLimits.resetsAt",
    [("time", clock(at, minutes >= 1440).as_str())],
  )
}

pub fn window(ctx: &Ctx, minutes: u64) -> Arc<str> {
  match minutes {
    300 => ctx.t("statusBar.planLimits.fiveHour"),
    10080 => ctx.t("statusBar.planLimits.weekly"),
    minutes if minutes % 1440 == 0 => ctx.t_args(
      "statusBar.planLimits.days",
      [("count", (minutes / 1440).to_string().as_str())],
    ),
    minutes => ctx.t_args(
      "statusBar.planLimits.hours",
      [("count", format!("{:.1}", minutes as f64 / 60.0).as_str())],
    ),
  }
}
