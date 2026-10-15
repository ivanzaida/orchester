//! `get_usage` on the `usage` seed: totals and rows grouped and filtered as the Usage page reads them, and
//! links the window's router opens as that page with the same query.

use orchester_app::routes::{ProjectPage, Route};
use orchester_workspace::links::with_usage_query;
use serde_json::{Value, json};

use super::Served;

fn routed(link: &Value) -> Route {
  let path = link.as_str().unwrap_or_else(|| panic!("a link: {link}"));
  Route::parse(path).unwrap_or_else(|error| panic!("{path} reads as a route: {error}"))
}

#[test]
fn domain_usage_answers_the_last_week_by_day_with_the_pages_link() {
  let (mut served, seeded) = Served::seeded("usage");
  let usage = served.ok("get_usage", &json!({ "project": "Kontur" }));
  assert_eq!(usage["group"], "day", "{usage}");
  assert_eq!(usage["total"]["runs"], 10, "{usage}");
  assert!(usage["total"]["tokens"]["spent"].as_u64().unwrap_or(0) > 0, "{usage}");
  let rows = usage["rows"].as_array().expect("rows");
  assert_eq!(rows.len(), 7, "one row per day: {usage}");
  assert_eq!(usage["link"], format!("/project/{}/usage", seeded.project));
  let Route::Project(project, ProjectPage::Usage(query)) = routed(&usage["link"]) else {
    panic!("the link opens the Usage page");
  };
  assert_eq!(project, seeded.project);
  assert_eq!(*query, Default::default());
  let today = rows.last().expect("today");
  let Route::Project(_, ProjectPage::Usage(narrowed)) = routed(&today["link"]) else {
    panic!("a day's link opens the Usage page");
  };
  assert_eq!(
    with_usage_query(String::new(), &narrowed),
    format!(
      "?from={day}&to={day}&group=hour",
      day = today["day"].as_str().expect("a day")
    ),
  );
}

#[test]
fn domain_usage_groups_and_filters_as_asked_and_links_each_row() {
  let (mut served, seeded) = Served::seeded("usage");
  let usage = served.ok(
    "get_usage",
    &json!({ "project": "Kontur", "group": "task", "model": "gpt-5.5" }),
  );
  let rows = usage["rows"].as_array().expect("rows");
  assert!(!rows.is_empty(), "{usage}");
  for row in rows {
    let key = row["task"].as_str().expect("a task row names its task");
    let Route::Project(project, ProjectPage::Usage(query)) = routed(&row["link"]) else {
      panic!("a task's link opens the Usage page");
    };
    assert_eq!(project, seeded.project);
    assert_eq!(query.filter.tasks, [key.into()]);
    assert_eq!(
      query.group,
      orchester_core::usage::GroupBy::Day,
      "the page groups by time"
    );
    assert_eq!(query.filter.models, ["gpt-5.5".into()]);
  }
  let agents = served.ok("get_usage", &json!({ "project": "Kontur", "group": "agent" }));
  let names: Vec<&str> = agents["rows"]
    .as_array()
    .expect("rows")
    .iter()
    .filter_map(|row| row["agent"]["name"].as_str())
    .collect();
  assert!(names.contains(&"Builder") && names.contains(&"Reviewer"), "{agents}");
  let everywhere = served.ok("get_usage", &json!({ "group": "pipeline" }));
  let Route::Usage(query) = routed(&everywhere["link"]) else {
    panic!("every project's usage opens All projects' Usage tab: {everywhere}");
  };
  assert_eq!(
    query.group,
    orchester_core::usage::GroupBy::Day,
    "the page groups by time"
  );
  let pipeline = &everywhere["rows"][0];
  assert!(
    matches!(routed(&pipeline["link"]), Route::Usage(narrowed) if !narrowed.filter.pipelines.is_empty()),
    "a row of every project's usage opens the tab narrowed to it: {pipeline}"
  );
  assert_eq!(everywhere["projects"][0]["name"], "Kontur", "{everywhere}");
}

#[test]
fn domain_usage_refuses_what_it_cannot_read() {
  let (mut served, _) = Served::seeded("usage");
  for arguments in [
    json!({ "project": "Kontur", "group": "week" }),
    json!({ "project": "Kontur", "from": "2026-10-15", "to": "2026-10-01" }),
    json!({ "project": "Kontur", "from": "yesterday" }),
    json!({ "project": "Kontur", "run": "KON-1" }),
    json!({ "project": "Kontur", "group": "hour", "from": "2026-09-01", "to": "2026-09-30" }),
    json!({ "project": "Kontur", "from": "2025-01-01", "to": "2026-01-02" }),
    json!({ "project": "Kontur", "model": "" }),
    json!({ "project": " " }),
  ] {
    let error = served.refused("get_usage", &arguments);
    assert_eq!(error["code"], "invalid_arguments", "{arguments}: {error}");
  }
}

#[test]
fn domain_usage_names_values_that_match_nothing_and_rounds_rows_to_the_total() {
  let (mut served, _) = Served::seeded("usage");
  let usage = served.ok(
    "get_usage",
    &json!({ "project": "Kontur", "group": "step", "agent": "builder", "task": "KON-999" }),
  );
  assert_eq!(
    usage["unmatched"],
    json!([{ "filter": "task", "value": "KON-999" }]),
    "an agent's name in any case matches: {usage}"
  );
  let agents = served.ok("get_usage", &json!({ "project": "Kontur", "group": "agent" }));
  let seconds: u64 = agents["rows"]
    .as_array()
    .expect("rows")
    .iter()
    .map(|row| row["active_seconds"].as_u64().expect("seconds"))
    .sum();
  assert_eq!(json!(seconds), agents["total"]["active_seconds"], "{agents}");
  assert_eq!(agents["unmatched"], json!([]));
}

#[test]
fn domain_usage_names_usage_on_no_account_and_filters_to_it() {
  let (mut served, _) = Served::seeded("usage");
  let accounts = served.ok("get_usage", &json!({ "project": "Kontur", "group": "account" }));
  let rows = accounts["rows"].as_array().expect("rows");
  let none = rows
    .iter()
    .find(|row| row["account"].is_null())
    .unwrap_or_else(|| panic!("a row of usage on no account: {accounts}"));
  let Route::Project(_, ProjectPage::Usage(query)) = routed(&none["link"]) else {
    panic!("its link opens the Usage page");
  };
  assert!(query.filter.no_account, "the link chooses No account: {}", none["link"]);
  let alone = served.ok("get_usage", &json!({ "project": "Kontur", "account": "none" }));
  assert_eq!(
    alone["total"]["tokens"]["spent"], none["tokens"]["spent"],
    "account=none totals what the no-account row has"
  );
  assert_eq!(alone["unmatched"], json!([]));
}
