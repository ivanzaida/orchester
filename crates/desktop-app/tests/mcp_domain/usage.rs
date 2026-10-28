//! `get_usage` on the `usage` seed: totals and rows grouped and filtered as the Usage page reads them, and
//! links the window's router opens as that page with the same query; `get_usage` and `get_run` count the
//! invocations whose output is not reported yet instead of passing their output off as 0.

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  connection::stream::AgentEvent,
  run::{InvocationKind, TokenUsage},
  store::{ActivityStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::links::with_usage_query;
use serde_json::{Value, json};

use super::{Served, kontur::Seeded};

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
fn domain_usage_names_each_provider_and_filters_to_it_by_name_or_id() {
  let (mut served, _) = Served::seeded("usage");
  let providers = served.ok("get_usage", &json!({ "project": "Kontur", "group": "provider" }));
  let rows = providers["rows"].as_array().expect("rows");
  assert!(
    rows.iter().all(|row| row.get("account").is_none()),
    "no row is an account's, and none is No account: {providers}"
  );
  let deepseek = rows
    .iter()
    .find(|row| row["provider"]["name"] == "Claude · DeepSeek")
    .unwrap_or_else(|| panic!("the API key's usage under its provider: {providers}"));
  assert_eq!(deepseek["provider"]["removed"], false);
  let Route::Project(_, ProjectPage::Usage(query)) = routed(&deepseek["link"]) else {
    panic!("its link opens the Usage page");
  };
  let id = deepseek["provider"]["id"].as_str().expect("its id");
  assert_eq!(query.filter.providers.len(), 1, "the link chooses the provider");
  assert_eq!(query.filter.providers[0].to_string(), id);
  for named in [id, "claude · deepseek"] {
    let alone = served.ok("get_usage", &json!({ "project": "Kontur", "provider": named }));
    assert_eq!(
      alone["total"]["tokens"]["spent"], deepseek["tokens"]["spent"],
      "provider={named} totals what its row has"
    );
    assert_eq!(alone["unmatched"], json!([]));
  }
  let earlier = rows
    .iter()
    .find(|row| row["provider"]["id"] == "earlier")
    .unwrap_or_else(|| panic!("the reviews recorded before providers were: {providers}"));
  let alone = served.ok("get_usage", &json!({ "project": "Kontur", "provider": "earlier" }));
  assert_eq!(alone["total"]["tokens"]["spent"], earlier["tokens"]["spent"]);
  let refused = served.refused("get_usage", &json!({ "project": "Kontur", "provider": "No account" }));
  assert_eq!(refused["code"], "invalid_arguments", "{refused}");
  let refused = served.refused("get_usage", &json!({ "project": "Kontur", "account": "none" }));
  assert_eq!(
    refused["code"], "invalid_arguments",
    "there is no account filter: {refused}"
  );
}

/// The run of `title` with its still open agent invocation now reporting a turn as Claude Code on DeepSeek's
/// Anthropic API does before its result: input, but output 0. Answers the run's id.
fn output_unreported_so_far(seeded: &Seeded, title: &str) -> String {
  let mut db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == title)
    .expect("the seeded task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  let mut invocation = db
    .invocations(run.id())
    .expect("invocations")
    .into_iter()
    .find(|invocation| invocation.ended_at.is_none() && matches!(invocation.kind, InvocationKind::Agent { .. }))
    .expect("the agent at work");
  let usage = TokenUsage {
    input: 60_000,
    cached_input: 50_000,
    output: 0,
  };
  db.append_activity(&invocation, &[(Timestamp::now(), AgentEvent::TurnCompleted { usage })])
    .expect("the turn is recorded");
  invocation.usage = usage;
  db.save_invocation(&invocation).expect("the invocation is saved");
  run.id().to_string()
}

#[test]
fn domain_usage_and_run_count_output_not_reported_yet_instead_of_a_false_zero() {
  let (mut served, seeded) = Served::seeded("usage");
  let usage = served.ok("get_usage", &json!({ "project": "Kontur" }));
  assert_eq!(
    usage["total"]["output_unreported"], 0,
    "every CLI reported its output: {usage}"
  );
  let before = usage["total"]["tokens"]["output"].clone();
  let run = output_unreported_so_far(&seeded, "Add retry to webhook sender");
  let shown = served.ok("get_run", &json!({ "project": "Kontur", "run": &run }));
  assert_eq!(shown["tokens"]["output_unreported"], 1, "{shown}");
  let usage = served.ok("get_usage", &json!({ "project": "Kontur" }));
  assert_eq!(
    usage["total"]["output_unreported"], 1,
    "the invocation's output is unknown, not 0: {usage}"
  );
  assert_eq!(
    usage["total"]["tokens"]["output"], before,
    "the output counts only what the others reported"
  );
  let alone = served.ok("get_usage", &json!({ "project": "Kontur", "run": run }));
  assert_eq!(alone["total"]["output_unreported"], 1, "{alone}");
}
