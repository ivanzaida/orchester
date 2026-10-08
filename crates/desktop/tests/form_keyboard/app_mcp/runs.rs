//! Runs through Orchester's MCP server, with the real runner and `orchester-fake-codex`: a draft is
//! queued and run, a second run is refused while one is active, the run is read, paused at its
//! checkpoint and resumed through its runner; a cancelled task runs again, a finished one does not.

use std::{fs, path::Path};

use orchester_core::{
  run::{Run, RunStatus},
  store::{ActivityStore, RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{agent, ok, refused};
use crate::{
  harness::Desktop,
  mcp_agent::McpAgent,
  task_run::{init_repository, programs, seed},
};

/// The latest run of task `key`, once it has one.
fn latest_of(repo: &Path, key: &str) -> Option<Run> {
  let db = ProjectDb::open(repo).expect("open the project");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)?;
  let latest = db.runs_of_task(task.id()).expect("runs").pop()?;
  Some(latest.expect("the latest run can be read"))
}

fn run_is(repo: &Path, key: &str, wanted: fn(&RunStatus) -> bool) -> bool {
  latest_of(repo, key).is_some_and(|run| wanted(run.status()))
}

/// The run's agent has started: its first invocation is recorded.
fn agent_at_work(repo: &Path, key: &str) -> bool {
  latest_of(repo, key).is_some_and(|run| {
    !ProjectDb::open(repo)
      .expect("open the project")
      .invocations(run.id())
      .expect("invocations")
      .is_empty()
  })
}

/// Creates task `title` on Delivery whose agent does what `brief` says, and runs it; its run's id.
fn create_and_run(agent: &mut McpAgent, title: &str, brief: &str) -> String {
  let created = ok(
    agent,
    "create_task",
    &json!({
      "project": "Kontur",
      "title": title,
      "brief": brief,
      "criteria": ["README.md has the agent's line"],
      "pipeline": "Delivery",
    }),
  );
  let key = created["task"]["key"].as_str().expect("a key");
  let started = ok(agent, "run_task", &json!({"project": "Kontur", "key": key}));
  started["run"]["id"].as_str().expect("a run").to_owned()
}

fn in_run(run: &str) -> Value {
  json!({"project": "Kontur", "run": run})
}

#[test]
fn app_mcp_runs_a_task_and_pauses_and_resumes_its_run() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| seed(repo, &fake, true));
  let repo = desktop.repo().to_path_buf();
  let mut agent = agent(&desktop);
  // Implement holds until released: a pause lets the active step finish, so one landing after
  // Implement would fall into Review, the last step, and the run would succeed instead.
  let run = create_and_run(&mut agent, "Say hello", "Wait for release.");
  let active = refused(&mut agent, "run_task", &json!({"project": "Kontur", "key": "KON-1"}));
  assert_eq!(active["code"], "run_active");

  desktop.wait_until("the agent works", |_| agent_at_work(&repo, "KON-1"));
  let read = ok(&mut agent, "get_run", &in_run(&run));
  assert_eq!(read["number"], 1);
  assert_eq!(read["task"], "KON-1");
  assert_eq!(read["pipeline"]["name"], "Delivery");
  assert_eq!(read["steps"][0]["name"], "Implement");
  assert_eq!(read["steps"][1]["name"], "Review");
  let by_number = ok(&mut agent, "get_run", &in_run("#1"));
  assert_eq!(by_number["id"], run.as_str());

  ok(&mut agent, "pause_run", &in_run(&run));
  fs::write(repo.join(".orchester").join("fake-release"), "go").expect("release the agent");
  desktop.wait_until("the run paused at its checkpoint", |_| {
    run_is(&repo, "KON-1", |status| matches!(status, RunStatus::Paused(_)))
  });
  let paused = ok(&mut agent, "get_run", &in_run(&run));
  assert_eq!(paused["status"]["state"], "paused");
  ok(&mut agent, "resume_run", &in_run(&run));
  desktop.wait_until("the run succeeded", |_| {
    run_is(&repo, "KON-1", |status| *status == RunStatus::Succeeded)
  });
  for control in ["pause_run", "resume_run", "cancel_run"] {
    let ended = refused(&mut agent, control, &in_run(&run));
    assert!(
      ended["code"].as_str().is_some_and(|code| code.starts_with("run_")),
      "{control}: {ended}"
    );
  }
  // Only a run that failed or was cancelled runs again.
  let finished = refused(&mut agent, "run_task", &json!({"project": "Kontur", "key": "KON-1"}));
  assert_eq!(finished["code"], "run_not_ended");
  let missing = refused(&mut agent, "get_run", &in_run("#9"));
  assert_eq!(missing["code"], "run_not_found");
}

#[test]
fn app_mcp_cancels_a_run_and_runs_its_task_again() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| seed(repo, &fake, false));
  let repo = desktop.repo().to_path_buf();
  let mut agent = agent(&desktop);
  let first = create_and_run(&mut agent, "Keep working", "Keep working.");
  desktop.wait_until("the agent works", |_| agent_at_work(&repo, "KON-1"));
  let cancelled = ok(&mut agent, "cancel_run", &in_run(&first));
  assert_eq!(cancelled["run"]["id"], first.as_str());
  desktop.wait_until("the run cancelled", |_| {
    run_is(&repo, "KON-1", |status| *status == RunStatus::Cancelled)
  });
  let again = ok(&mut agent, "run_task", &json!({"project": "Kontur", "key": "KON-1"}));
  let second = again["run"]["id"].as_str().expect("a run").to_owned();
  assert_ne!(second, first);
  let read = ok(&mut agent, "get_run", &in_run(&second));
  assert_eq!(read["number"], 2);
  ok(&mut agent, "cancel_run", &in_run(&second));
  desktop.wait_until("the second run cancelled", |_| {
    run_is(&repo, "KON-1", |status| *status == RunStatus::Cancelled)
  });
}
