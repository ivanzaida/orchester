//! The stand-in's `/api/v2` reads of the releases: the release list, a release, its progress, its burndown, its
//! history and its changelog. Progress and burndown are counted by core's module rules over the release's tasks, each
//! at the workflow status it has now (the stand-in keeps no status history); with no delivery history, the forecast
//! is `done` or `not_enough_history`. Lists are answered in one page.

use chrono::NaiveDate;
use orchester_core::{
  forecast::{Forecast, Outlook},
  id::{ReleaseId, TaskId},
  module::{
    burndown::{Burndown, ChartDays, TaskTrack},
    measure::WorkUnit,
  },
  task::organization::WorkflowStatus,
  time::Timestamp,
  usage::local_date,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Kept, error, parse, release_of};
use crate::{
  test_support::team_server::{
    State,
    authority::{capabilities, listing},
  },
  v2::{EventCursor, ForecastRead, PageRequest, ReleaseBurndownRead, ReleaseListRequest},
  wire,
};

#[derive(Deserialize)]
struct Workflow(#[serde(with = "wire::Workflow")] WorkflowStatus);

/// A release read of the project `(id, key)`, `route` being its method and what follows the project: the status and
/// the answer.
pub(in crate::test_support::team_server) fn read(
  project: (&str, &str),
  (method, route, body): (&str, &str, &str),
  state: &State,
) -> (u16, Value) {
  let cursor = state.authority.cursor().to_string();
  if (method, route) == ("POST", "release-list") {
    return match parse::<ReleaseListRequest>(body) {
      Ok(request) => {
        let releases = state.authority.releases.kept.iter();
        let kept = releases.filter(|kept| request.state.is_none_or(|wanted| kept.release.state() == wanted));
        let items: Vec<Value> = kept.map(|kept| summary(project, kept, state)).collect();
        (200, json!({"items": items, "next": null, "cursor": cursor}))
      }
      Err(refusal) => refusal,
    };
  }
  let rest = route.strip_prefix("releases/").unwrap_or_default();
  let (id, part) = rest.split_once('/').unwrap_or((rest, ""));
  let Some(kept) = id
    .parse::<ReleaseId>()
    .ok()
    .and_then(|id| state.authority.releases.get(id))
  else {
    return (404, error("not_found"));
  };
  match (method, part) {
    ("GET", "") => (200, summary(project, kept, state)),
    ("GET", "progress") => (200, summary(project, kept, state)["progress"].clone()),
    ("GET", "burndown") => (200, burndown(project, kept, state)),
    ("GET", "changelog") => (200, json!(kept.changelog())),
    ("POST", "history") => match parse::<PageRequest>(body) {
      Ok(_) => {
        let id = Some(kept.release.id);
        let changes = state.authority.releases.changes.iter();
        let items: Vec<_> = changes.filter(|change| change.from == id || change.to == id).collect();
        (200, json!({"items": items, "next": null, "cursor": cursor}))
      }
      Err(refusal) => refusal,
    },
    _ => (404, error("not_found")),
  }
}

/// The tasks of `project` in the release `id`.
fn inside(project: (&str, &str), id: ReleaseId, state: &State) -> Vec<Value> {
  let tasks = listing::tasks(project, state);
  tasks.into_iter().filter(|task| release_of(task) == Some(id)).collect()
}

/// The forecast of `remaining` tasks with no delivery history.
fn forecast(remaining: u32) -> Forecast {
  Forecast {
    remaining,
    history_days: 0,
    history_delivered: 0,
    outlook: if remaining == 0 {
      Outlook::Done
    } else {
      Outlook::NotEnoughHistory
    },
  }
}

/// `kept` as a `ReleaseSummary`.
fn summary(project: (&str, &str), kept: &Kept, state: &State) -> Value {
  let tasks = inside(project, kept.release.id, state);
  let count =
    |which: &dyn Fn(&Value) -> bool| u32::try_from(tasks.iter().filter(|task| which(task)).count()).unwrap_or(u32::MAX);
  let scope = count(&|task| task["workflow"] != "canceled");
  let done = count(&|task| task["workflow"] == "done");
  let forecast = forecast(scope - done);
  let today = local_date(Timestamp::now());
  let planned = kept.planned_day();
  let after_planned = match (&forecast.outlook, planned) {
    (Outlook::Dates { safe, .. }, Some(planned)) => kept.release.released.is_none() && *safe > planned,
    _ => false,
  };
  let overdue = planned
    .filter(|planned| kept.release.released.is_none() && *planned < today)
    .map(|planned| (today - planned).num_days());
  json!({
    "release": {"release": kept.release, "capabilities": capabilities(project.0, state.authority.standing)},
    "progress": {"unit": "tasks", "scope": scope, "done": done}, "tasks": count(&|_| true),
    "forecast": ForecastRead::from_core(&forecast), "after_planned": after_planned, "overdue_days": overdue,
  })
}

/// `kept`'s burndown from the day it was made through its planned day or today, over every task that was ever in it.
fn burndown(project: (&str, &str), kept: &Kept, state: &State) -> Value {
  let id = Some(kept.release.id);
  let tasks = listing::tasks(project, state);
  let changes = &state.authority.releases.changes;
  let tracks: Vec<TaskTrack> = tasks
    .iter()
    .filter_map(|task| {
      let task_id = task["id"].as_str()?.parse::<TaskId>().ok()?;
      let membership: Vec<(Timestamp, bool)> = changes
        .iter()
        .filter(|change| change.task == task_id && (change.from == id || change.to == id))
        .map(|change| (Timestamp::from_unix_millis(change.at), change.to == id))
        .collect();
      let status = serde_json::from_value::<Workflow>(task["workflow"].clone()).ok()?.0;
      (!membership.is_empty()).then_some(TaskTrack {
        task: task_id,
        weight: 1,
        membership,
        first_status: status,
        statuses: Vec::new(),
      })
    })
    .collect();
  let days = ChartDays {
    first: local_date(Timestamp::from_unix_millis(kept.release.created.at)),
    target: kept.planned_day(),
    today: local_date(Timestamp::now()),
  };
  let chart = Burndown::of(&tracks, WorkUnit::Tasks, days);
  let remaining = chart.latest().and_then(|day| day.remaining).unwrap_or(0);
  let cursor = EventCursor::new(&state.authority.cursor().to_string()).expect("the stand-in's cursors are positive");
  json!(ReleaseBurndownRead::from_core(
    kept.release.id,
    (&chart, &forecast(remaining)),
    cursor
  ))
}

impl Kept {
  fn planned_day(&self) -> Option<NaiveDate> {
    let planned = self.release.fields.planned.as_deref()?;
    NaiveDate::parse_from_str(planned, "%Y-%m-%d").ok()
  }
}
