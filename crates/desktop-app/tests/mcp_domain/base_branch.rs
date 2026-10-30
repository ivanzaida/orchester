//! A task's and a module's base branch through the domain tools (ORC-407): `create_module`/`update_module` and
//! `create_task`/`update_task` set it, a name only a remote has creating its local branch and a name the repository
//! has nowhere refused before anything is saved; `get_task`, `list_modules` and `get_module` read it, the task with
//! the branch its next run takes and where it comes from; `get_run` gives the base a run recorded and its source.

use kontur::Seeded;
use orchester_core::{
  run::BranchName,
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{Served, kontur, live::git};

/// The decisions scenario, whose checkout is on `main`, with a local branch `staging` and `release` only on the
/// remote `origin`.
fn served() -> (Served, Seeded) {
  let (served, seeded) = Served::seeded("decisions");
  let repo = seeded.repo.as_path();
  git(repo, &["branch", "staging"]);
  git(repo, &["remote", "add", "origin", "https://example.invalid/kontur.git"]);
  git(repo, &["update-ref", "refs/remotes/origin/release", "HEAD"]);
  (served, seeded)
}

/// The `base_branch` of `task` as `get_task` reads it.
fn task_base(served: &mut Served, task: &Value) -> Value {
  served.ok("get_task", &json!({ "project": "Kontur", "key": task }))["base_branch"].clone()
}

#[test]
fn domain_base_branch_of_a_module_is_set_read_and_a_remote_name_makes_its_branch() {
  let (mut served, seeded) = served();
  let repo = seeded.repo.as_path();
  let made = served.ok(
    "create_module",
    &json!({ "project": "Kontur", "name": "Release train", "base_branch": "origin/release" }),
  );
  assert_eq!(made["module"]["base_branch"], "release", "{made}");
  let module = made["module"]["id"].clone();
  assert_eq!(
    git(repo, &["rev-parse", "release"]),
    git(repo, &["rev-parse", "origin/release"]),
    "the remote-only name made its local branch"
  );
  assert_eq!(git(repo, &["symbolic-ref", "--short", "HEAD"]).trim(), "main");
  let refused = served.refused(
    "create_module",
    &json!({ "project": "Kontur", "name": "Never made", "base_branch": "nowhere" }),
  );
  assert_eq!(refused["code"], "branch_not_found", "{refused}");

  let changed = served.ok(
    "update_module",
    &json!({ "project": "Kontur", "module": module, "base_branch": "checked_out", "state": "closed" }),
  );
  assert!(changed["module"]["base_branch"].is_null(), "{changed}");
  assert_eq!(changed["module"]["state"], "closed");
  let rebased = served.ok(
    "update_module",
    &json!({ "project": "Kontur", "module": module, "base_branch": "staging" }),
  );
  assert_eq!(
    rebased["module"]["state"], "closed",
    "the base branch alone changed: {rebased}"
  );

  let listed = served.ok("list_modules", &json!({ "project": "Kontur" }));
  let modules = listed["modules"].as_array().expect("modules");
  assert!(
    modules.iter().all(|entry| entry["module"]["name"] != "Never made"),
    "the refused module was never made: {listed}"
  );
  let entry = modules
    .iter()
    .find(|entry| entry["module"]["id"] == module)
    .unwrap_or_else(|| panic!("the module is listed: {listed}"));
  assert_eq!(entry["module"]["base_branch"], "staging");
  let page = served.ok("get_module", &json!({ "project": "Kontur", "module": module }));
  assert_eq!(page["module"]["base_branch"], "staging", "{page}");
}

#[test]
fn domain_base_branch_of_a_task_is_set_and_read_with_its_next_run() {
  let (mut served, _seeded) = served();
  let made = served.ok(
    "create_module",
    &json!({ "project": "Kontur", "name": "Release train", "base_branch": "release" }),
  );
  let module = made["module"]["id"].clone();
  let created = served.ok(
    "create_task",
    &json!({ "project": "Kontur", "title": "Cut the release", "module": "Release train" }),
  );
  assert_eq!(
    created["task"]["base_branch"],
    json!({ "choice": "inherit", "branch": null })
  );
  let key = created["task"]["key"].clone();
  let read = task_base(&mut served, &key);
  assert_eq!(read["next_run"]["branch"], "release", "{read}");
  assert_eq!(read["next_run"]["source"], "module");
  assert_eq!(read["next_run"]["module"], module);

  let updated = served.ok(
    "update_task",
    &json!({ "project": "Kontur", "key": key, "base_branch": "staging" }),
  );
  assert_eq!(
    updated["task"]["base_branch"],
    json!({ "choice": "branch", "branch": "staging" })
  );
  let read = task_base(&mut served, &key);
  assert_eq!(
    (read["next_run"]["branch"].clone(), read["next_run"]["source"].clone()),
    (json!("staging"), json!("task")),
    "{read}"
  );

  served.ok(
    "update_task",
    &json!({ "project": "Kontur", "key": key, "base_branch": "checked_out" }),
  );
  let read = task_base(&mut served, &key);
  assert_eq!(read["choice"], "checked_out", "{read}");
  assert_eq!(
    (read["next_run"]["branch"].clone(), read["next_run"]["source"].clone()),
    (Value::Null, json!("checked_out"))
  );

  let gone = served.refused(
    "update_task",
    &json!({ "project": "Kontur", "key": key, "base_branch": "nowhere", "title": "Renamed" }),
  );
  assert_eq!(gone["code"], "branch_not_found", "{gone}");
  let task = served.ok("get_task", &json!({ "project": "Kontur", "key": key }));
  assert_eq!(
    task["title"], "Cut the release",
    "nothing of the refused update was saved"
  );
  let refused = served.refused(
    "create_task",
    &json!({ "project": "Kontur", "title": "Never made", "base_branch": "nowhere" }),
  );
  assert_eq!(refused["code"], "branch_not_found", "{refused}");
  let listed = served.ok("list_tasks", &json!({ "project": "Kontur", "search": "Never made" }));
  assert_eq!(listed["tasks"].as_array().map(Vec::len), Some(0), "{listed}");
}

#[test]
fn domain_base_branch_get_run_gives_the_base_the_run_recorded_and_its_source() {
  let (mut served, seeded) = served();
  let db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let run = db
    .tasks()
    .expect("tasks")
    .iter()
    .find_map(|task| db.runs_of_task(task.id()).expect("runs").pop())
    .expect("the scenario has a run")
    .expect("readable");
  let shown = served.ok(
    "get_run",
    &json!({ "project": "Kontur", "run": run.number().get().to_string() }),
  );
  assert_eq!(shown["base"]["source"], "checked_out", "{shown}");
  assert_eq!(shown["base"]["revision"], run.base().as_str());
  assert_eq!(
    shown["base"]["branch"],
    json!(run.base_branch().map(BranchName::as_str))
  );
  assert!(shown["base"]["module"].is_null());
}
