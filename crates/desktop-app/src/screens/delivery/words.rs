//! How the Delivery view words its figures: durations in days and hours, hours to a tenth, a period's name and a
//! forecast's dates or why it has none. Days and short durations are worded as the Usage page words them
//! (`screens::usage::words`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_workspace::delivery::{DeliveryPeriod, Forecast, Outlook};

use crate::screens::usage::words::{self as usage_words, day};

const HOUR_MILLIS: u64 = 3_600_000;

/// A duration as `2d 5h` from a day on; shorter, in hours and minutes as the Usage page words agent time (`7h 20m`,
/// `45m`).
pub fn duration(ctx: &Ctx, millis: u64) -> Arc<str> {
  let hours = millis / HOUR_MILLIS;
  if hours < 24 {
    return usage_words::duration(ctx, millis);
  }
  ctx.t_args(
    "delivery.duration.days",
    [
      ("days", (hours / 24).to_string().as_str()),
      ("hours", (hours % 24).to_string().as_str()),
    ],
  )
}

/// Hours to a tenth, `3.5 h`, as a chart's point reads them.
pub fn hours(ctx: &Ctx, millis: u64) -> Arc<str> {
  let tenths = millis * 10 / HOUR_MILLIS;
  let value = format!("{}.{}", tenths / 10, tenths % 10);
  ctx.t_args("delivery.count.hours", [("value", value.as_str())])
}

pub fn period(ctx: &Ctx, period: DeliveryPeriod) -> Arc<str> {
  ctx.t(match period {
    DeliveryPeriod::Month => "delivery.period.month",
    DeliveryPeriod::Quarter => "delivery.period.quarter",
    DeliveryPeriod::Year => "delivery.period.year",
  })
}

/// A forecast's 50% and 85% words: its dates, *Not enough history yet* and nothing, or *Nothing left* and nothing.
pub fn outlook(ctx: &Ctx, forecast: &Forecast) -> (Arc<str>, Arc<str>) {
  let none = ctx.t("delivery.cycle.outside");
  match &forecast.outlook {
    Outlook::Done => (ctx.t("delivery.forecast.nothingLeft"), none),
    Outlook::NotEnoughHistory => (ctx.t("delivery.forecast.notEnough"), none),
    Outlook::Dates {
      likely,
      safe,
      past_horizon,
    } => {
      let safe = if *past_horizon {
        ctx.t("delivery.forecast.pastHorizon")
      } else {
        day(ctx, *safe)
      };
      (day(ctx, *likely), safe)
    }
  }
}

/// How much history a forecast rests on, `24 days · 9 merged`.
pub fn history(ctx: &Ctx, forecast: &Forecast) -> Arc<str> {
  ctx.t_args(
    "delivery.forecast.history",
    [
      ("days", forecast.history_days.to_string().as_str()),
      ("tasks", forecast.history_delivered.to_string().as_str()),
    ],
  )
}
