//! The tools on a team project and on a local one.

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::ConnectionId,
  name::Name,
  store::{ConnectionStore, TaskStore},
  time::Timestamp,
};
use orchester_store::TeamStartStage;
use serde_json::json;

use crate::{
  Checkout,
  fixture::Config,
  reads,
  team_server::{MEMBER, TeamServer},
};

/// The fake team server with the tools' routes, and what they saw.
fn server() -> (TeamServer, std::sync::Arc<parking_lot::Mutex<reads::Seen>>) {
  let config = Config::new("Say hello.");
  let (extra, seen) = reads::routes(&config);
  (TeamServer::start_with(config, Some(extra)), seen)
}

#[test]
fn team_mcp_reads_come_from_the_team_server() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);

  let task = checkout
    .call("get_task", &json!({"project": "Kontur", "key": "kon-7"}))
    .expect("read");
  assert_eq!(task["title"], "Say hello");
  assert_eq!(task["id"], server.task.to_string());
  assert_eq!(task["module"], server.module.to_string());
  assert_eq!(task["revision"], 1);
  assert_eq!(task["team"], true);
  let by_id = checkout
    .call(
      "get_task",
      &json!({"project": "Kontur", "key": server.task.to_string()}),
    )
    .expect("read by id");
  assert_eq!(by_id["key"], "KON-7");

  let listed = checkout
    .call("list_tasks", &json!({"project": "Kontur"}))
    .expect("listed");
  let keys: Vec<&str> = listed["tasks"]
    .as_array()
    .expect("tasks")
    .iter()
    .filter_map(|task| task["key"].as_str())
    .collect();
  assert_eq!(keys, ["KON-7"]);
  let module = checkout
    .call("get_module", &json!({"project": "Kontur", "module": "webhooks"}))
    .expect("the module");
  assert_eq!(module["branch"], "module/webhooks");
  assert_eq!(module["module"]["id"], server.module.to_string());

  // Across projects, a team project's tasks are not read from this computer's copy: it is listed apart.
  let across = checkout.call("list_tasks", &json!({})).expect("listed");
  assert_eq!(across["tasks"], json!([]));
  assert_eq!(
    across["unreadable_projects"][0]["error"]["code"],
    "team_project_unsupported"
  );
  assert!(
    checkout.db().tasks().expect("tasks").is_empty(),
    "no copy of the team's tasks"
  );
}

#[test]
fn team_mcp_member_edit_is_refused_as_not_admin_and_changes_nothing() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);

  let edit = json!({"project": "Kontur", "key": "KON-7", "title": "Say goodbye"});
  assert_eq!(checkout.refused("update_task", &edit), "not_admin");
  assert_eq!(
    seen.lock().task_edits,
    1,
    "the edit went to the task's definition route"
  );
  let create = json!({"project": "Kontur", "title": "Say goodbye"});
  assert_eq!(checkout.refused("create_task", &create), "not_admin");
  assert_eq!(seen.lock().task_creates, 1, "the new task went to task.create");
  let task = checkout
    .call("get_task", &json!({"project": "Kontur", "key": "KON-7"}))
    .expect("read");
  assert_eq!(task["title"], "Say hello");
  assert!(checkout.db().tasks().expect("tasks").is_empty(), "nothing saved here");

  // A run's or a person's workflow status, the pipeline and the limits wait for the team's decisions: not sent.
  let done = json!({"project": "Kontur", "key": "KON-7", "workflow_status": "done"});
  assert_eq!(checkout.refused("update_task", &done), "decision_required");
  let budget = json!({"project": "Kontur", "key": "KON-7", "budget_tokens": 1000});
  assert_eq!(checkout.refused("update_task", &budget), "decision_required");
  let points = json!({"project": "Kontur", "key": "KON-7", "story_points": 3});
  assert_eq!(checkout.refused("update_task", &points), "team_field_unsupported");
  assert_eq!(seen.lock().task_edits, 1, "none of them reached the server");
  let draft = json!({"project": "Kontur", "key": "KON-7"});
  assert_eq!(checkout.refused("return_to_draft", &draft), "decision_required");
}

#[test]
fn team_mcp_run_task_on_a_team_task_goes_through_work_start() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);

  // The session has no runner here: the start is accepted and kept, and waits for the runner.
  let code = checkout.refused("run_task", &json!({"project": "Kontur", "key": "KON-7"}));
  assert_eq!(code, "runner_unavailable");
  {
    let state = server.state();
    assert_eq!(state.works_made, 1, "work.start made the task's work");
    assert!(
      state.claim.as_ref().is_some_and(|claim| claim.token == MEMBER),
      "claimed for this computer's session"
    );
  }
  let starts = checkout.db().open_team_starts().expect("starts");
  assert_eq!(starts.len(), 1);
  assert_eq!(
    starts[0].stage,
    TeamStartStage::InputsStored,
    "the accepted inputs are kept"
  );
  assert!(
    checkout.db().tasks().expect("tasks").is_empty(),
    "no task definition kept"
  );

  // A second Codex connection leaves the Builder profile's connection to the caller.
  let second = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex work").expect("name"),
    AgentProvider::CodexCli,
    checkout.root().join("codex-work.exe"),
    Timestamp::now(),
  )
  .expect("connection");
  checkout.db().create_connection(&second).expect("a connection");
  let code = checkout.refused("run_task", &json!({"project": "Kontur", "key": "KON-7"}));
  assert_eq!(code, "bindings_required");
  assert_eq!(server.state().works_made, 1, "nothing more was started");

  // A team task's run goes into its module's branch, never merged into the base branch here.
  let merge = json!({"project": "Kontur", "run": "1"});
  assert_eq!(checkout.refused("merge_run", &merge), "team_delivery_pending");
}

#[test]
fn team_mcp_no_sql_fallback_while_the_server_is_unavailable() {
  let (server, _seen) = server();
  // A draft saved before the project was linked: a task this computer's database still holds.
  let checkout = Checkout::new(Some(&server), &["Say hello"]);
  assert_eq!(checkout.db().tasks().expect("tasks").len(), 1);

  let local = json!({"project": "Kontur", "key": "KON-1"});
  assert_eq!(
    checkout.refused("get_task", &local),
    "task_not_found",
    "only the server's tasks are read"
  );

  server.set_down(true);
  let task = json!({"project": "Kontur", "key": "KON-7"});
  assert_eq!(checkout.refused("get_task", &task), "team_unavailable");
  assert_eq!(checkout.refused("get_task", &local), "team_unavailable");
  assert_eq!(
    checkout.refused("list_tasks", &json!({"project": "Kontur"})),
    "team_unavailable"
  );
  assert_eq!(
    checkout.refused("list_modules", &json!({"project": "Kontur"})),
    "team_unavailable"
  );
  assert_eq!(checkout.refused("run_task", &task), "team_unavailable");
  assert_eq!(server.state().works_made, 0, "nothing started");
  let edit = json!({"project": "Kontur", "key": "KON-7", "title": "Say goodbye"});
  assert_eq!(checkout.refused("update_task", &edit), "team_unavailable");
  assert_eq!(
    checkout.db().tasks().expect("tasks")[0].details().title.as_str(),
    "Say hello",
    "nothing changed here"
  );
}

#[test]
fn team_mcp_local_projects_are_unchanged() {
  let checkout = Checkout::new(None, &["Sign in"]);

  let task = checkout
    .call("get_task", &json!({"project": "Kontur", "key": "KON-1"}))
    .expect("read");
  assert_eq!(task["title"], "Sign in");
  assert!(task.get("team").is_none());
  let created = checkout
    .call("create_task", &json!({"project": "Kontur", "title": "Sign out"}))
    .expect("created");
  assert_eq!(created["task"]["key"], "KON-2");
  let updated = checkout
    .call(
      "update_task",
      &json!({"project": "Kontur", "key": "KON-2", "title": "Sign out everywhere", "workflow_status": "todo"}),
    )
    .expect("updated");
  assert_eq!(updated["task"]["workflow_status"], "todo");
  let listed = checkout
    .call("list_tasks", &json!({"project": "Kontur"}))
    .expect("listed");
  assert_eq!(listed["tasks"].as_array().expect("tasks").len(), 2);
  let titles: Vec<String> = checkout
    .db()
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.details().title.as_str().to_owned())
    .collect();
  assert_eq!(titles, ["Sign in", "Sign out everywhere"]);
  let bindings = json!({"project": "Kontur", "key": "KON-1", "bindings": [{"profile": "x", "connection": "Codex"}]});
  assert_eq!(checkout.refused("run_task", &bindings), "invalid_arguments");
}
