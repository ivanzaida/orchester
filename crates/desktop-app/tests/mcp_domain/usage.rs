//! `get_usage` on the `usage` seed: totals and rows grouped and filtered as the Usage page reads them, and
//! links the window's router opens as that page with the same query.

use orchester_app::routes::{ProjectPage, Route};
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
    narrowed.query_string(),
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
  assert_eq!(
    everywhere["link"],
    Value::Null,
    "every project's usage has no one page: {everywhere}"
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
  ] {
    let error = served.refused("get_usage", &arguments);
    assert_eq!(error["code"], "invalid_arguments", "{arguments}: {error}");
  }
}
