//! The S10 reads of releases, the task list's release filter and the S11 release scopes.

use chrono::NaiveDate;
use orchester_api::v2::{
  EventCursor, OutlookRead, QueryScope, ReleaseBurndownRead, ReleaseHistoryPage, ReleaseListRequest, ReleaseSummary,
  TaskFilters,
};
use orchester_core::{
  forecast::{Forecast, Outlook},
  id::{ProjectId, ReleaseId, TaskId, UserId},
  module::{
    burndown::{Burndown, BurndownDay},
    measure::WorkUnit,
  },
  release::ReleaseState,
};
use serde_json::{Value, json};

use super::{refuses_unknown_and_missing, roundtrip};

fn release(id: ReleaseId) -> Value {
  let by = json!({"id": UserId::generate(), "display_name": "Olha"});
  json!({
    "id": id, "name": "0.4.0", "description": "", "planned": "2026-11-15", "state": "planned", "released_on": null,
    "released_by": null, "released_commit": null, "revision": 4, "created": {"by": by, "at": 1},
    "updated": {"by": by, "at": 2},
  })
}

fn capabilities() -> Value {
  json!({"project_id": ProjectId::generate(), "revision": 1, "role": "admin",
    "actions": [{"action": "read", "subject": null, "allowed": true, "reason": null}], "unresolved": []})
}

#[test]
fn release_list_request_filters_by_state() {
  let request: ReleaseListRequest = serde_json::from_value(json!({})).expect("everything defaults");
  assert_eq!(request, ReleaseListRequest::default());
  assert_eq!(request.state, None);
  let released: ReleaseListRequest =
    serde_json::from_value(json!({"state": "released", "page": {"limit": 20}})).expect("a request");
  assert_eq!(released.state, Some(ReleaseState::Released));
  assert_eq!(released.page.limit(), 20);
  for invalid in [json!({"state": "open"}), json!({"order": "planned"})] {
    assert!(serde_json::from_value::<ReleaseListRequest>(invalid).is_err());
  }
}

#[test]
fn release_summary_carries_progress_and_its_forecast_against_the_planned_day() {
  let id = ReleaseId::generate();
  let wire = json!({
    "release": {"release": release(id), "capabilities": capabilities()},
    "progress": {"unit": "tasks", "scope": 7, "done": 4}, "tasks": 8,
    "forecast": {"remaining": 3, "history_days": 30, "history_delivered": 9,
      "outlook": {"kind": "dates", "likely": "2026-11-17", "safe": "2026-11-20", "past_horizon": false}},
    "after_planned": true, "overdue_days": null,
  });
  let summary: ReleaseSummary = serde_json::from_value(wire.clone()).expect("a summary");
  assert_eq!(summary.release.release.id, id);
  assert_eq!(
    (summary.progress.scope, summary.progress.done, summary.tasks),
    (7, 4, 8)
  );
  assert!(summary.after_planned);
  assert!(matches!(summary.forecast.outlook, OutlookRead::Dates { .. }));
  roundtrip(&summary, &wire);
  for field in ["overdue_days", "after_planned", "forecast"] {
    refuses_unknown_and_missing::<ReleaseSummary>(&wire, field);
  }
  let mut overdue = wire;
  overdue["overdue_days"] = json!(3);
  assert_eq!(
    serde_json::from_value::<ReleaseSummary>(overdue)
      .expect("overdue")
      .overdue_days,
    Some(3)
  );
}

#[test]
fn release_burndown_and_history_read_by_core() {
  let (id, task) = (ReleaseId::generate(), TaskId::generate());
  let day = NaiveDate::from_ymd_opt(2026, 11, 2).expect("a day");
  let burndown = Burndown {
    unit: WorkUnit::Tasks,
    days: vec![BurndownDay {
      date: day,
      scope: Some(2),
      remaining: Some(1),
      ideal: None,
      added: 2,
      removed: 0,
    }],
    start_scope: 2,
  };
  let forecast = Forecast {
    remaining: 1,
    history_days: 0,
    history_delivered: 0,
    outlook: Outlook::NotEnoughHistory,
  };
  let cursor = EventCursor::new("12").expect("a cursor");
  let read = ReleaseBurndownRead::from_core(id, (&burndown, &forecast), cursor);
  let wire = json!({"release": id, "unit": "tasks", "start_scope": 2,
    "days": [{"date": "2026-11-02", "scope": 2, "remaining": 1, "ideal": null, "added": 2, "removed": 0}],
    "forecast": {"remaining": 1, "history_days": 0, "history_delivered": 0, "outlook": {"kind": "not_enough_history"}},
    "cursor": "12"});
  roundtrip(&read, &wire);
  refuses_unknown_and_missing::<ReleaseBurndownRead>(&wire, "release");

  let by = json!({"id": UserId::generate(), "display_name": "Olha"});
  let history = json!({"items": [{"sequence": 3, "task": task, "from": null, "to": id, "by": by, "actor": "person",
    "at": 9}], "next": null, "cursor": "12"});
  let page: ReleaseHistoryPage = serde_json::from_value(history.clone()).expect("a history page");
  assert_eq!(page.items[0].to, Some(id));
  roundtrip(&page, &history);
}

#[test]
fn task_filters_and_scopes_name_releases() {
  let id = ReleaseId::generate();
  let filters: TaskFilters = serde_json::from_value(json!({"releases": [id, null]})).expect("filters");
  assert_eq!(filters.releases, vec![Some(id), None]);
  assert_eq!(TaskFilters::default().releases, Vec::new());

  roundtrip(&QueryScope::ReleaseList {}, &json!({"kind": "release_list"}));
  roundtrip(
    &QueryScope::Release { release: id },
    &json!({"kind": "release", "release": id}),
  );
  assert!(serde_json::from_value::<QueryScope>(json!({"kind": "release_list", "release": id})).is_err());
  assert!(serde_json::from_value::<QueryScope>(json!({"kind": "release"})).is_err());
}
