//! The domain tools listed beside lurq's and the app's, behind the server's token; reading and changing the
//! seeded project as its screens do; typed refusals; setup and pipeline tools checked as their forms check.

use std::path::Path;

use serde_json::{Value, json};

use super::{Served, post, titles};

#[test]
fn domain_tools_are_listed_beside_lurqs_and_the_apps_with_their_scopes() {
  let (served, _) = Served::seeded("empty");
  let tools = served.tools();
  let named = |name: &str| tools.iter().find(|tool| tool["name"] == name).cloned();
  for tool in [
    "app_state",
    "lurq_screenshot",
    "list_projects",
    "get_task",
    "run_task",
    "decide_approval",
    "add_project",
    "add_cli_connection",
    "add_mcp_server",
    "update_mcp_server",
    "remove_mcp_server",
    "set_mcp_server_enabled",
    "save_profile_draft",
    "save_pipeline_draft",
    "save_instruction_draft",
    "merge_run",
  ] {
    assert!(named(tool).is_some(), "{tool} is listed");
  }
  assert!(
    named("navigate").is_none(),
    "window control stays off while the data tools are on"
  );
  let read_only = |name: &str| named(name).expect("listed")["annotations"]["readOnlyHint"] == true;
  assert!(read_only("list_projects") && read_only("get_pipeline"));
  assert!(!read_only("create_task") && !read_only("merge_run"));
  let schema = &named("add_cli_connection").expect("listed")["inputSchema"];
  assert_eq!(schema["required"], json!(["project", "kind", "name"]));
}

#[test]
fn domain_tools_need_the_servers_token() {
  let (served, _) = Served::seeded("empty");
  let call = json!({
    "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "list_projects", "arguments": {} },
  });
  let response = post(served.port, &[("Mcp-Session-Id", &served.session)], &call);
  assert_eq!(response.status, 401);
  let response = post(
    served.port,
    &[
      ("Authorization", "Bearer not-the-token"),
      ("Mcp-Session-Id", &served.session),
    ],
    &call,
  );
  assert_eq!(response.status, 401);
}

#[test]
fn domain_list_projects_tasks_and_a_task_read_the_seeded_project() {
  let (mut served, seeded) = Served::seeded("tasks");
  let projects = served.ok("list_projects", &json!({}));
  assert_eq!(projects["projects"][0]["name"], "Kontur");
  assert_eq!(projects["projects"][0]["id"], seeded.project.to_string());
  let tasks = served.ok("list_tasks", &json!({ "project": "Kontur" }));
  let keys: Vec<&str> = tasks["tasks"]
    .as_array()
    .expect("tasks")
    .iter()
    .filter_map(|task| task["key"].as_str())
    .collect();
  for task in &seeded.tasks {
    assert!(keys.contains(&task.key.as_str()), "{} is listed: {keys:?}", task.key);
  }
  let first = &seeded.tasks[0];
  let task = served.ok("get_task", &json!({ "project": "kontur", "key": first.key }));
  assert_eq!(task["title"], first.title);
  let missing = served.refused("get_task", &json!({ "project": "Kontur", "key": "KON-999" }));
  assert_eq!(missing["code"], "task_not_found");
  let unknown = served.refused("list_tasks", &json!({ "project": "Nowhere" }));
  assert_eq!(unknown["code"], "project_not_found");
}

#[test]
fn domain_create_task_makes_the_feed_read_again() {
  let (mut served, seeded) = Served::seeded("empty");
  let feed = served.tasks_feed(seeded.project);
  served.until("the first reading", |_| titles(&feed).is_some());
  let created = served.ok(
    "create_task",
    &json!({ "project": "Kontur", "title": "Retry with jitter", "brief": "Back off with jitter." }),
  );
  assert_eq!(created["task"]["submission"], "draft");
  served.until("the feed to read the new task", |_| {
    titles(&feed).is_some_and(|titles| titles.iter().any(|title| title == "Retry with jitter"))
  });
}

#[test]
fn domain_run_task_without_a_runner_is_a_typed_refusal() {
  // Any file stands in for the CLI: the seed records its check, and the runner never starts it.
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  let (mut served, _) = Served::seeded_with("empty", Some(codex));
  let task = json!({
    "project": "Kontur",
    "title": "Retry with jitter",
    "brief": "Back off with jitter.",
    "criteria": ["Retries are spread out."],
    "pipeline": "Quick fix",
  });
  let created = served.ok("create_task", &task);
  let key = created["task"]["key"].as_str().expect("a key").to_owned();
  let refused = served.refused("run_task", &json!({ "project": "Kontur", "key": key }));
  assert_eq!(refused["code"], "runner_unavailable", "{refused}");
  assert!(
    !refused["reasons"].as_array().expect("reasons").is_empty(),
    "the app's own words say why"
  );
}

#[test]
fn domain_automation_answers_approvals_only_while_a_person_lets_it() {
  let (mut served, _) = Served::seeded("empty");
  let refused = served.refused(
    "decide_approval",
    &json!({ "project": "Kontur", "approval": "any", "decision": "approve_once" }),
  );
  assert_eq!(refused["code"], "automation_not_allowed");
  let connection = served.ok("list_connections", &json!({ "project": "Kontur" }));
  assert!(connection["agents"].is_array());
}

#[test]
fn domain_setup_tools_check_their_input_as_the_forms_do() {
  let (mut served, seeded) = Served::seeded("empty");
  let folder = seeded.repo.display().to_string();
  let again = served.refused("add_project", &json!({ "name": "Kontur again", "folder": folder }));
  assert_eq!(
    again["code"], "already_listed",
    "the list holds Kontur in the folder: {again}"
  );
  let relative = served.refused("add_project", &json!({ "name": "Elsewhere", "folder": "repo" }));
  assert_eq!(relative["code"], "folder_not_absolute");
  let blank = served.refused("add_project", &json!({ "name": " ", "folder": folder }));
  assert_eq!(blank["code"], "invalid_name");
  let not_a_repository = served.folder.path().join("plain");
  std::fs::create_dir_all(&not_a_repository).expect("a plain folder");
  let plain = served.refused(
    "add_project",
    &json!({ "name": "Plain", "folder": not_a_repository.display().to_string() }),
  );
  assert_eq!(plain["code"], "not_a_git_repository");
  let unborn = served.folder.path().join("unborn");
  std::fs::create_dir_all(&unborn).expect("a folder");
  let init = std::process::Command::new("git")
    .arg("-C")
    .arg(&unborn)
    .args(["init", "--quiet"])
    .output()
    .expect("git runs");
  assert!(init.status.success(), "git init");
  let no_commits = served.refused(
    "add_project",
    &json!({ "name": "Unborn", "folder": unborn.display().to_string() }),
  );
  assert_eq!(
    no_commits["code"], "no_commits",
    "runs need a commit to branch from: {no_commits}"
  );
  let unknown = served.refused(
    "save_profile_draft",
    &json!({ "project": "Kontur", "role": "Builder", "connection": "nowhere", "model": "gpt-5.5" }),
  );
  assert_eq!(unknown["code"], "connection_not_found");
  let environment = served.refused(
    "set_project_environment",
    &json!({ "project": "Kontur", "variables": [{ "name": "1BAD", "value": "x" }] }),
  );
  assert_eq!(environment["code"], "invalid_variable");
  let saved = served.ok(
    "set_project_environment",
    &json!({ "project": "Kontur", "variables": [{ "name": "RUST_LOG", "value": "info" }] }),
  );
  assert_eq!(saved["environment"], json!([{ "name": "RUST_LOG", "value": "info" }]));
  let files = served.ok(
    "set_repository_agent_files",
    &json!({ "project": "Kontur", "enabled": false }),
  );
  assert_eq!(files["repository_agent_files"], false);
  let project = served.ok("get_project", &json!({ "project": "Kontur" }));
  assert_eq!(project["project"]["key"], "KON");
  assert_eq!(project["project"]["repository_agent_files"], false);
  let unknown_argument = served.refused("get_project", &json!({ "project": "Kontur", "secret": "x" }));
  assert_eq!(unknown_argument["code"], "invalid_arguments");
}

#[test]
fn domain_pipelines_are_made_saved_and_refused_as_the_editor_does() {
  let (mut served, _) = Served::seeded("empty");
  let empty = served.ok(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "Hotfix", "from": "empty" }),
  );
  assert_eq!(empty["pipeline"]["state"], "draft_only");
  let taken = served.refused(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "hotfix", "from": "empty" }),
  );
  assert_eq!(taken["code"], "name_taken");
  let starter = served.ok(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "Delivery", "from": "starter" }),
  );
  let keys: Vec<&str> = starter["pipeline"]["steps"]
    .as_array()
    .expect("steps")
    .iter()
    .filter_map(|step| step["key"].as_str())
    .collect();
  assert_eq!(
    keys,
    [
      "research",
      "plan",
      "implement",
      "run-checks",
      "review",
      "create-pull-request"
    ]
  );
  let cycle = served.refused(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Hotfix", "steps": [
      { "key": "a", "name": "A", "runs_after": ["b"] },
      { "key": "b", "name": "B", "runs_after": ["a"] },
    ] }),
  );
  assert_eq!(cycle["code"], "invalid_pipeline", "{cycle}");
  let saved = served.ok(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Hotfix", "description": "Small fixes", "steps": [
      { "key": "fix", "name": "Fix", "budget": 50000 },
    ] }),
  );
  assert_eq!(saved["pipeline"]["steps"][0]["budget"], 50000);
  assert_eq!(saved["pipeline"]["description"], "Small fixes");
  let first = json!({ "project": "Kontur", "pipeline": "Hotfix" });
  assert_eq!(
    served.refused("publish_pipeline", &first)["code"],
    "person_only",
    "a pipeline's first version is a person's"
  );
  served.let_automation_widen(true);
  let unassigned = served.refused("publish_pipeline", &first);
  assert_eq!(unassigned["code"], "not_publishable");
  served.let_automation_widen(false);
  let write = served.refused(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Hotfix", "steps": [
      { "key": "fix", "name": "Fix", "repository": "write" },
    ] }),
  );
  assert_eq!(
    write["code"], "invalid_step",
    "a step cannot write without a profile that writes"
  );
}

/// `save_pipeline_draft` keeps an agent step's choice of whether it merges the base branch when that moves (ORC-65);
/// a step that leaves it out follows the default rule, and a tool step cannot choose.
#[test]
fn domain_save_pipeline_draft_keeps_a_steps_choice_about_base_moves() {
  let (mut served, _) = Served::seeded("empty");
  served.ok(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "Hotfix", "from": "empty" }),
  );
  let saved = served.ok(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Hotfix", "steps": [
      { "key": "fix", "name": "Fix" },
      { "key": "verify", "name": "Verify", "runs_after": ["fix"], "takes_base_moves": true },
    ] }),
  );
  let steps = &saved["pipeline"]["steps"];
  assert_eq!(steps[0]["takes_base_moves"], Value::Null, "{saved}");
  assert_eq!(
    steps[0]["merges_moved_base"], false,
    "a read-only step does not by default"
  );
  assert_eq!(steps[1]["takes_base_moves"], true, "{saved}");
  assert_eq!(
    steps[1]["merges_moved_base"], true,
    "the step's choice overrides the default"
  );
  let opened = served.ok("get_pipeline", &json!({ "project": "Kontur", "pipeline": "Hotfix" }));
  assert_eq!(
    opened["pipeline"]["steps"][1]["takes_base_moves"], true,
    "kept in the draft"
  );
  let tool = served.refused(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Hotfix", "steps": [
      { "key": "check", "name": "Check", "tool": "Check", "takes_base_moves": true },
    ] }),
  );
  assert_eq!(tool["code"], "invalid_step", "{tool}");
}
