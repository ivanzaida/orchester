//! Tasks and pipelines through Orchester's MCP server: a draft saved with the new task form's rules and
//! the project's default limits, read back, found by the list's search and status filter, changed,
//! and shown on the task list, which reads the tasks again as after its own saves.

use serde_json::json;

use super::{agent, ok, refused};
use crate::{harness::Desktop, mcp_agent::McpAgent, task_run::seed};

/// A project with the published pipeline Delivery (one Implement step on a Builder).
fn seeded() -> Desktop {
  Desktop::with_seeded_project_and_app_mcp(|repo| seed(repo, &repo.join("orchester-fake-codex"), false))
}

#[test]
fn app_mcp_lists_the_published_pipelines() {
  let desktop = seeded();
  let mut agent = agent(&desktop);
  let listed = ok(&mut agent, "list_pipelines", &json!({"project": "Kontur"}));
  let pipelines = listed["pipelines"].as_array().expect("pipelines");
  assert_eq!(pipelines.len(), 1);
  assert_eq!(pipelines[0]["name"], "Delivery");
  assert_eq!(pipelines[0]["version"], 1);
  assert_eq!(pipelines[0]["steps"][0]["name"], "Implement");
  assert_eq!(pipelines[0]["steps"][0]["approval_gate"], false);
  // Without a project, every project the app knows, each pipeline naming its project.
  let everywhere = ok(&mut agent, "list_pipelines", &json!({}));
  assert_eq!(everywhere["pipelines"][0]["project"]["name"], "Kontur");
  let error = refused(
    &mut agent,
    "list_pipelines",
    &json!({"project": "Kontur", "status": "draft"}),
  );
  assert_eq!(error["code"], "invalid_arguments");
}

#[test]
fn app_mcp_creates_finds_and_changes_tasks() {
  let mut desktop = seeded();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  let mut agent = agent(&desktop);

  let created = ok(
    &mut agent,
    "create_task",
    &json!({
      "project": "Kontur",
      "title": "Add keyboard shortcuts",
      "brief": "Ctrl+K opens the project switcher.",
      "criteria": ["Ctrl+K opens the switcher", "  "],
      "pipeline": "delivery",
      "priority": "high",
    }),
  );
  assert_eq!(created["task"]["key"], "KON-1");
  assert_eq!(created["task"]["submission"], "draft");
  assert_eq!(created["task"]["priority"], "high");
  assert_eq!(created["task"]["workflow_status"], "backlog");
  // The task list reads the tasks again, as after a save of its own.
  desktop.wait_for("task-row-KON-1");

  let task = ok(&mut agent, "get_task", &json!({"project": "Kontur", "key": "kon-1"}));
  assert_eq!(task["title"], "Add keyboard shortcuts");
  assert_eq!(task["criteria"], json!(["Ctrl+K opens the switcher"]));
  assert_eq!(task["phase"], "draft");
  assert!(
    task["pipeline"]["shown"]
      .as_str()
      .is_some_and(|shown| shown.contains("Delivery"))
  );
  // The project's defaults: no budget and no time limit.
  assert_eq!(
    task["limits"],
    json!({"budget_tokens": null, "time_limit_minutes": null})
  );
  assert_eq!(task["runs"], json!([]));

  let changed = ok(
    &mut agent,
    "update_task",
    &json!({
      "project": "Kontur",
      "key": "KON-1",
      "title": "Add switcher shortcuts",
      "workflow_status": "todo",
      "priority": "urgent",
      "budget_tokens": 2_000_000,
      "time_limit_minutes": 45,
    }),
  );
  assert_eq!(changed["task"]["workflow_status"], "todo");
  let task = ok(&mut agent, "get_task", &json!({"project": "Kontur", "key": "KON-1"}));
  assert_eq!(task["title"], "Add switcher shortcuts");
  assert_eq!(task["brief"], "Ctrl+K opens the project switcher.");
  assert_eq!(task["priority"], "urgent");
  assert_eq!(
    task["limits"],
    json!({"budget_tokens": 2_000_000, "time_limit_minutes": 45})
  );
  // `null` lifts a limit; the other stays.
  ok(
    &mut agent,
    "update_task",
    &json!({"project": "Kontur", "key": "KON-1", "budget_tokens": null}),
  );
  let task = ok(&mut agent, "get_task", &json!({"project": "Kontur", "key": "KON-1"}));
  assert_eq!(task["limits"], json!({"budget_tokens": null, "time_limit_minutes": 45}));

  finds_the_changed_task(&mut agent);
}

#[test]
fn app_mcp_refuses_tasks_the_screens_refuse() {
  let desktop = seeded();
  let mut agent = agent(&desktop);
  let blank = refused(&mut agent, "create_task", &json!({"project": "Kontur", "title": "  "}));
  assert_eq!(blank["code"], "invalid_task");
  assert!(blank["message"].as_str().is_some_and(|message| !message.is_empty()));
  let budget = refused(
    &mut agent,
    "create_task",
    &json!({"project": "Kontur", "title": "Too small", "budget_tokens": 0}),
  );
  assert_eq!(budget["code"], "invalid_task");
  let pipeline = refused(
    &mut agent,
    "create_task",
    &json!({"project": "Kontur", "title": "Unknown pipeline", "pipeline": "Release"}),
  );
  assert_eq!(pipeline["code"], "pipeline_not_found");
  let missing = refused(&mut agent, "get_task", &json!({"project": "Kontur", "key": "KON-99"}));
  assert_eq!(missing["code"], "task_not_found");
  let missing = refused(
    &mut agent,
    "update_task",
    &json!({"project": "Kontur", "key": "KON-99"}),
  );
  assert_eq!(missing["code"], "task_not_found");

  // A draft without a pipeline cannot run; the app says what it still needs.
  ok(
    &mut agent,
    "create_task",
    &json!({"project": "Kontur", "title": "No pipeline"}),
  );
  let incomplete = refused(&mut agent, "run_task", &json!({"project": "Kontur", "key": "KON-1"}));
  assert_eq!(incomplete["code"], "task_incomplete");
}

/// The list's search and its workflow status filter find KON-1, now To do.
fn finds_the_changed_task(agent: &mut McpAgent) {
  let found = ok(agent, "list_tasks", &json!({"project": "Kontur", "search": "switcher"}));
  assert_eq!(found["tasks"][0]["key"], "KON-1");
  assert_eq!(found["tasks"][0]["workflow_status"], "todo");
  let todo = ok(agent, "list_tasks", &json!({"project": "Kontur", "status": "todo"}));
  assert_eq!(todo["tasks"].as_array().map(Vec::len), Some(1));
  let done = ok(agent, "list_tasks", &json!({"project": "Kontur", "status": "done"}));
  assert_eq!(done["tasks"], json!([]));
}
