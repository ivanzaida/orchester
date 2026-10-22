//! `get_delivery`: how delivery goes in a project, as its Delivery view reads it (`crate::delivery`): throughput,
//! cycle and lead time, where agent time goes, cumulative flow over the last days, and when the queued work and each
//! open module are likely delivered, with the history the forecast rests on. Reading only; nothing is money.

use orchester_core::{
  id::ProjectId,
  time::Timestamp,
  usage::{DayRange, local_date},
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{modules::find_module, wire};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  delivery::{
    CycleTimes, Delivery, DeliveryPeriod, FLOW_STATUSES, Forecast, Outlook, Spread, WhereTimeGoes, Work, WorkForecast,
    forecast::{HISTORY_DAYS, MIN_DELIVERED, MIN_HISTORY_DAYS},
    last_days,
  },
  links::{Link, TaskPage},
};

/// The most days `get_delivery` reads: a year and a day, as `get_usage`.
const LONGEST_DAYS: u64 = 366;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetDelivery {
  /// The project's id, or its name.
  pub project: String,
  /// How many local days the series cover, today included: 30 when left out, at most 366.
  #[serde(default)]
  pub days: Option<u64>,
  /// A module's id or name: only its forecast is given, beside the queued work's.
  #[serde(default)]
  pub module: Option<String>,
}

pub async fn get_delivery(services: &Services, args: GetDelivery) -> Result<Value, ToolError> {
  let days = args.days.unwrap_or(DeliveryPeriod::default().days());
  if !(1..=LONGEST_DAYS).contains(&days) {
    return Err(ToolError::InvalidArguments(format!(
      "days must be 1 to {LONGEST_DAYS}, not {days}"
    )));
  }
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let module = args
      .module
      .as_deref()
      .map(|named| find_module(&db, named))
      .transpose()?;
    let now = Timestamp::now();
    let range = last_days(days, local_date(now));
    let mut delivery = Delivery::read(&db, range, now)?;
    if let Some(module) = module {
      delivery.forecasts.retain(|forecast| match &forecast.work {
        Work::Queued => true,
        Work::Module { id, .. } => *id == module.id(),
      });
    }
    Ok(delivery_json(open.project.id(), &delivery, days))
  })
  .await
}

fn delivery_json(project: ProjectId, delivery: &Delivery, days: u64) -> Value {
  let period = DeliveryPeriod::ALL
    .into_iter()
    .find(|period| period.days() == days)
    .unwrap_or_default();
  json!({
    "range": range_json(delivery.range),
    "unit": "tasks",
    "throughput": throughput_json(delivery),
    "cycle_time": cycle_json(project, &delivery.cycle),
    "where_time_goes": time_json(&delivery.time),
    "cumulative_flow": flow_json(delivery),
    "forecasts": delivery.forecasts.iter().map(|forecast| forecast_json(project, forecast)).collect::<Vec<_>>(),
    "forecast_rule": format!(
      "Monte Carlo over the tasks merged on each of the last {HISTORY_DAYS} whole days (not before the project's first task); \
       no dates with fewer than {MIN_HISTORY_DAYS} days or fewer than {MIN_DELIVERED} tasks merged in them"
    ),
    "unread_runs": delivery.unread_runs,
    "link": Link::Delivery(project, period).path(),
  })
}

fn range_json(range: DayRange) -> Value {
  json!({ "first_day": range.first().to_string(), "last_day": range.last().to_string() })
}

fn throughput_json(delivery: &Delivery) -> Value {
  let bars: Vec<Value> = delivery
    .throughput
    .iter()
    .map(|bar| {
      json!({
        "first_day": bar.first.to_string(),
        "last_day": bar.last.to_string(),
        "tasks": bar.tasks,
        "points": bar.points,
        "tasks_without_points": bar.unpointed,
      })
    })
    .collect();
  let by = match delivery.bucketing {
    crate::delivery::Bucketing::Day => "day",
    crate::delivery::Bucketing::Week => "week",
  };
  json!({ "by": by, "bars": bars })
}

/// Milliseconds as hours, to a tenth.
fn hours(ms: u64) -> f64 {
  (ms as f64 / 360_000.0).round() / 10.0
}

fn spread_json(spread: Option<Spread>) -> Value {
  spread.map_or(Value::Null, |spread| {
    json!({
      "tasks": spread.count,
      "median_hours": hours(spread.median),
      "p25_hours": hours(spread.lower),
      "p75_hours": hours(spread.upper),
      "p85_hours": hours(spread.p85),
    })
  })
}

fn cycle_json(project: ProjectId, cycle: &CycleTimes) -> Value {
  let tasks: Vec<Value> = cycle
    .rows
    .iter()
    .map(|row| {
      json!({
        "key": &*row.key,
        "title": &*row.title,
        "merged_at": wire::time(row.merged_at),
        "cycle_hours": row.cycle_ms.map(hours),
        "lead_hours": hours(row.lead_ms),
        "attempts": row.attempts,
        "link": row.key.parse().ok().map(|key| Link::Task(project, key, TaskPage::Summary).path()),
      })
    })
    .collect();
  json!({ "cycle": spread_json(cycle.cycle), "lead": spread_json(cycle.lead), "tasks": tasks })
}

fn time_json(time: &WhereTimeGoes) -> Value {
  let steps: Vec<Value> = time
    .steps
    .iter()
    .map(|step| {
      json!({
        "step": &*step.step,
        "active_hours": hours(step.active_ms),
        "executions": step.executions,
        "returned": step.returned,
        "return_percent": step.return_percent(),
      })
    })
    .collect();
  json!({
    "steps": steps,
    "delivered_tasks": time.delivered,
    "attempts_per_delivered_task": time.attempts_per_task(),
  })
}

fn flow_json(delivery: &Delivery) -> Value {
  let statuses: Vec<Value> = FLOW_STATUSES
    .iter()
    .map(|status| json!(wire::Workflow::from(*status)))
    .collect();
  let days: Vec<Value> = delivery
    .flow
    .iter()
    .map(|day| json!({ "day": day.date.to_string(), "counts": day.counts }))
    .collect();
  json!({ "statuses": statuses, "days": days })
}

fn forecast_json(project: ProjectId, forecast: &WorkForecast) -> Value {
  let work = match &forecast.work {
    Work::Queued => json!({ "kind": "queued_tasks" }),
    Work::Module { id, name } => json!({
      "kind": "module",
      "id": id.to_string(),
      "name": &**name,
      "link": Link::Module(project, *id).path(),
    }),
  };
  json!({ "work": work, "remaining": forecast.forecast.remaining, "forecast": outlook_json(&forecast.forecast) })
}

/// A forecast's dates, or why it has none, with the history it rests on; `get_module` gives a module's so.
pub(super) fn outlook_json(forecast: &Forecast) -> Value {
  let history = json!({ "days": forecast.history_days, "tasks_merged": forecast.history_delivered });
  match &forecast.outlook {
    Outlook::Done => json!({ "state": "nothing_left", "history": history }),
    Outlook::NotEnoughHistory => json!({ "state": "not_enough_history", "history": history }),
    Outlook::Dates {
      likely,
      safe,
      past_horizon,
    } => json!({
      "state": "dates",
      "likely_50": likely.to_string(),
      "likely_85": safe.to_string(),
      "past_ten_years": past_horizon,
      "history": history,
    }),
  }
}
