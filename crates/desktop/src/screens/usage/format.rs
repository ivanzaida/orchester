//! How the dashboard words its figures: compact tokens by the display rule, exact grouped counts in
//! tooltips and tables, days and ranges, changes against the previous period (never good or bad)
//! and agent time.

use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::AgentProvider,
  time::Timestamp,
  tokens::{compact_tokens, grouped},
  usage::{Change, CliSeries, DayRange, RunOutcome, local_date},
};

use crate::display::{MONTH_KEYS, WEEKDAY_KEYS};

const MINUTE_MILLIS: u64 = 60_000;
const HOUR_MILLIS: u64 = 60 * MINUTE_MILLIS;

/// A token count by the display rule (`31.6M`).
pub fn tokens(count: u64) -> Arc<str> {
  compact_tokens(count).into()
}

/// A count grouped for the locale (`412,345`).
pub fn exact(ctx: &Ctx, count: u64) -> Arc<str> {
  grouped(count, &ctx.t("number.groupSeparator")).into()
}

/// `Sep 24`.
pub fn day(ctx: &Ctx, date: NaiveDate) -> Arc<str> {
  ctx.t_args(
    "usage.date.day",
    [
      ("month", &*ctx.t(MONTH_KEYS[date.month0() as usize])),
      ("day", date.day().to_string().as_str()),
    ],
  )
}

/// `Mon 29`, as a bar and a tooltip name a day.
pub fn weekday(ctx: &Ctx, date: NaiveDate) -> Arc<str> {
  ctx.t_args(
    "usage.date.weekday",
    [
      (
        "weekday",
        &*ctx.t(WEEKDAY_KEYS[date.weekday().num_days_from_monday() as usize]),
      ),
      ("day", date.day().to_string().as_str()),
    ],
  )
}

/// `Wed, Sep 24`, as a table names a day.
pub fn long_day(ctx: &Ctx, date: NaiveDate) -> Arc<str> {
  ctx.t_args(
    "usage.date.longDay",
    [
      (
        "weekday",
        &*ctx.t(WEEKDAY_KEYS[date.weekday().num_days_from_monday() as usize]),
      ),
      ("day", &*day(ctx, date)),
    ],
  )
}

/// The period a change is against: *yesterday* before today, `Sep 17 – 23` within a month.
pub fn previous_name(ctx: &Ctx, previous: DayRange, today: NaiveDate) -> Arc<str> {
  if previous.days() == 1 && previous.first().succ_opt() == Some(today) {
    return ctx.t("usage.date.yesterday");
  }
  let (first, last) = (previous.first(), previous.last());
  if first != last && first.month() == last.month() && first.year() == last.year() {
    return ctx.t_args(
      "usage.date.rangeInMonth",
      [("from", &*day(ctx, first)), ("to", last.day().to_string().as_str())],
    );
  }
  range(ctx, previous)
}

/// When a report came, near now: `today 15:58`, `yesterday 19:42`, else `Oct 4, 09:00`.
pub fn reported(ctx: &Ctx, at: Timestamp, now: Timestamp) -> Arc<str> {
  let (date, today) = (local_date(at), local_date(now));
  let time = crate::display::clock(at);
  if date == today {
    ctx.t_args("usage.date.todayAt", [("time", time.as_str())])
  } else if date.succ_opt() == Some(today) {
    ctx.t_args("usage.date.yesterdayAt", [("time", time.as_str())])
  } else {
    date_time(ctx, at)
  }
}

/// A reset time: the clock alone today (`17:40`), else with its day (`Oct 4, 09:00`).
pub fn reset_time(ctx: &Ctx, at: Timestamp, now: Timestamp) -> Arc<str> {
  if local_date(at) == local_date(now) {
    crate::display::clock(at).into()
  } else {
    date_time(ctx, at)
  }
}

/// `Sep 24 – Sep 30`, or one day.
pub fn range(ctx: &Ctx, range: DayRange) -> Arc<str> {
  if range.first() == range.last() {
    return day(ctx, range.first());
  }
  ctx.t_args(
    "usage.date.range",
    [("from", &*day(ctx, range.first())), ("to", &*day(ctx, range.last()))],
  )
}

/// A local hour, `09:00`; times are numbers, not words.
pub fn hour(hour: u32) -> String {
  format!("{hour:02}:00")
}

/// A local date and time, `Oct 4, 09:00`.
pub fn date_time(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  ctx.t_args(
    "usage.date.dateTime",
    [
      ("day", &*day(ctx, local_date(at))),
      ("time", crate::display::clock(at).as_str()),
    ],
  )
}

/// A local weekday and time, `Fri 26, 14:00`.
pub fn weekday_time(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  ctx.t_args(
    "usage.date.dateTime",
    [
      ("day", &*weekday(ctx, local_date(at))),
      ("time", crate::display::clock(at).as_str()),
    ],
  )
}

/// Active agent time: `18 h 40 min`, `45 min`, or under a minute.
pub fn duration(ctx: &Ctx, millis: u64) -> Arc<str> {
  let (hours, minutes) = (millis / HOUR_MILLIS, millis % HOUR_MILLIS / MINUTE_MILLIS);
  match (hours, minutes) {
    (0, 0) if millis > 0 => ctx.t("usage.duration.underMinute"),
    (0, minutes) => ctx.t_args("usage.duration.minutes", [("minutes", minutes.to_string().as_str())]),
    (hours, minutes) => ctx.t_args(
      "usage.duration.hours",
      [
        ("hours", hours.to_string().as_str()),
        ("minutes", minutes.to_string().as_str()),
      ],
    ),
  }
}

/// What a change counts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeUnit {
  Percent,
  Points,
  Count,
}

/// A change against the period named `range`: words and the direction's icon, never a color.
pub fn change(ctx: &Ctx, change: Option<Change>, range: &str, unit: ChangeUnit) -> (Option<bool>, Arc<str>) {
  let one = matches!(change, Some(Change::More(1) | Change::Less(1)));
  let (more, less) = match unit {
    ChangeUnit::Percent => ("usage.change.percent.more", "usage.change.percent.less"),
    ChangeUnit::Points if one => ("usage.change.points.oneMore", "usage.change.points.oneLess"),
    ChangeUnit::Points => ("usage.change.points.more", "usage.change.points.less"),
    ChangeUnit::Count => ("usage.change.count.more", "usage.change.count.less"),
  };
  match change {
    Some(Change::More(amount)) => (
      Some(true),
      ctx.t_args(more, [("amount", amount.to_string().as_str()), ("range", range)]),
    ),
    Some(Change::Less(amount)) => (
      Some(false),
      ctx.t_args(less, [("amount", amount.to_string().as_str()), ("range", range)]),
    ),
    Some(Change::Same) => (None, ctx.t_args("usage.change.same", [("range", range)])),
    Some(Change::NoBaseline) => (None, ctx.t_args("usage.change.noBaseline", [("range", range)])),
    None => (None, ctx.t_args("usage.change.unknown", [("range", range)])),
  }
}

/// The CLI a series stands for.
pub fn series_name(ctx: &Ctx, series: CliSeries) -> Arc<str> {
  ctx.t(match series {
    CliSeries::Codex => "usage.cli.codex",
    CliSeries::Claude => "usage.cli.claude",
    CliSeries::Other => "usage.cli.other",
  })
}

pub fn cli_name(ctx: &Ctx, cli: AgentProvider) -> Arc<str> {
  ctx.t(match cli {
    AgentProvider::CodexCli => "usage.cli.codex",
    AgentProvider::ClaudeCodeCli => "usage.cli.claude",
    AgentProvider::AnthropicApi => "usage.cli.anthropic",
    AgentProvider::OpenAiApi => "usage.cli.openai",
  })
}

/// A run's outcome, as its status label reads.
pub const fn outcome_key(outcome: RunOutcome) -> &'static str {
  match outcome {
    RunOutcome::Pending => "usage.outcome.pending",
    RunOutcome::Running => "usage.outcome.running",
    RunOutcome::Paused => "usage.outcome.paused",
    RunOutcome::Waiting => "usage.outcome.waiting",
    RunOutcome::Interrupted => "usage.outcome.interrupted",
    RunOutcome::Succeeded => "usage.outcome.succeeded",
    RunOutcome::Failed => "usage.outcome.failed",
    RunOutcome::Cancelled => "usage.outcome.cancelled",
  }
}

/// A share in whole percent, or a dash when it is not known.
pub fn share(ctx: &Ctx, percent: Option<u64>) -> Arc<str> {
  percent.map_or_else(
    || ctx.t("usage.unknown"),
    |percent| ctx.t_args("usage.percent", [("value", percent.to_string().as_str())]),
  )
}
