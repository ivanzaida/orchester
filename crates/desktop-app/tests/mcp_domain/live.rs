//! A project set up end to end through the domain tools alone, then worked with the real runner and
//! the fake Codex: the project added from a new git repository, a connection to the fake checked and
//! saved, a profile and a pipeline published, a task created and run, its agent's request answered by
//! automation once a person lets it, and its work merged into main. Automation never approves what
//! auto-approval leaves to a person, such as a command that reaches the network.

use std::{
  env, fs,
  path::{Path, PathBuf},
  process::Command,
  sync::OnceLock,
};

use orchester_workspace::app_paths::runner_beside;
use serde_json::{Value, json};

use super::Served;

/// The runner and the fake Codex, built into this profile's folder once per test process, before a
/// test starts them (Windows keeps a running program's file).
fn programs() -> (PathBuf, PathBuf) {
  static BUILT: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();
  BUILT.get_or_init(build_programs).clone()
}

fn build_programs() -> (PathBuf, PathBuf) {
  let status = Command::new(env!("CARGO"))
    .args([
      "build",
      "--quiet",
      "-p",
      "orchester-runner",
      "--features",
      "test-agent",
      "--bins",
    ])
    .status()
    .expect("run cargo");
  assert!(status.success(), "cargo build -p orchester-runner: {status}");
  let test = env::current_exe().expect("the test's path");
  let profile = test
    .parent()
    .and_then(Path::parent)
    .expect("tests live in <target>/<profile>/deps");
  let fake = profile
    .join("orchester-fake-codex")
    .with_extension(env::consts::EXE_EXTENSION);
  (runner_beside(&profile.join("orchester-app")), fake)
}

fn git(repo: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A new git repository on `main` with one commit, and the app on a data folder beside it.
fn served() -> (Served, PathBuf, PathBuf) {
  let (runner, fake) = programs();
  let folder = tempfile::tempdir().expect("a temporary folder");
  let repo = folder.path().join("kontur");
  fs::create_dir_all(&repo).expect("the repository's folder");
  git(&repo, &["init", "--quiet", "--initial-branch=main"]);
  git(&repo, &["config", "user.name", "Orchester Test"]);
  git(&repo, &["config", "user.email", "test@orchester.invalid"]);
  git(&repo, &["config", "commit.gpgsign", "false"]);
  fs::write(repo.join("README.md"), "hi\n").expect("the readme");
  git(&repo, &["add", "README.md"]);
  git(&repo, &["commit", "--quiet", "-m", "initial"]);
  let database = folder.path().join("bin").join("orchester.db");
  fs::create_dir_all(folder.path().join("bin")).expect("the data folder");
  (Served::start(folder, &database, runner), repo, fake)
}

/// Kontur set up through the tools: its connection to the fake Codex, a Builder profile and the
/// Delivery pipeline, both published.
fn set_up(served: &mut Served, repo: &Path, fake: &Path) {
  let added = served.ok(
    "add_project",
    &json!({ "name": "Kontur", "folder": repo.display().to_string() }),
  );
  assert_eq!(added["project"]["key"], "KON");
  let connection = served.ok(
    "add_cli_connection",
    &json!({ "project": "Kontur", "kind": "codex", "name": "Codex · Kontur", "program": fake.display().to_string() }),
  );
  assert_eq!(connection["check"]["verified"], true, "{connection}");
  assert_eq!(connection["connection"]["status"], "connected");
  let models = served.ok(
    "list_models",
    &json!({ "project": "Kontur", "connection": "Codex · Kontur" }),
  );
  assert_eq!(models["models"][0]["id"], "gpt-5.5");
  let offered = served.refused(
    "save_profile_draft",
    &json!({ "project": "Kontur", "role": "Builder", "connection": "Codex · Kontur", "model": "gpt-5.5", "effort": "extreme" }),
  );
  assert_eq!(offered["code"], "effort_not_offered");
  let profile = served.ok(
    "save_profile_draft",
    &json!({
      "project": "Kontur",
      "role": "Builder",
      "responsibilities": "Implements the task.",
      "connection": "Codex · Kontur",
      "model": "gpt-5.5",
      "effort": "medium",
      "permissions": { "repository": "write", "commands": "any_in_worktree", "network": "on" },
    }),
  );
  assert_eq!(profile["profile"]["state"]["state"], "draft");
  let published = served.ok("publish_profile", &json!({ "project": "Kontur", "profile": "Builder" }));
  assert_eq!(published["profile"]["state"]["published_version"], 1);
  served.ok(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "Delivery", "from": "empty" }),
  );
  served.ok(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Delivery", "steps": [
      { "key": "implement", "name": "Implement", "profile": "Builder", "repository": "write", "instructions": "Append one line." },
    ] }),
  );
  let pipeline = served.ok(
    "publish_pipeline",
    &json!({ "project": "Kontur", "pipeline": "Delivery" }),
  );
  assert_eq!(pipeline["pipeline"]["published_version"], 1, "{pipeline}");
}

/// Creates and runs a task with `brief` on Delivery; its key.
fn run(served: &mut Served, brief: &str) -> String {
  let created = served.ok(
    "create_task",
    &json!({
      "project": "Kontur",
      "title": "Say hello",
      "brief": brief,
      "criteria": ["The readme says hello."],
      "pipeline": "Delivery",
    }),
  );
  let key = created["task"]["key"].as_str().expect("a key").to_owned();
  served.ok("run_task", &json!({ "project": "Kontur", "key": key }));
  key
}

/// The first request waiting for a decision, once the agent asked.
fn waiting_approval(served: &mut Served) -> Value {
  let mut approval = Value::Null;
  served.until("the agent's request", |served| {
    let listed = served.ok("list_approvals", &json!({ "project": "Kontur" }));
    approval = listed["approvals"][0].clone();
    !approval.is_null()
  });
  approval
}

/// A person turns on Let automation answer approvals.
fn let_automation_answer(served: &Served) {
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("the setting is saved");
}

#[test]
fn domain_a_project_set_up_through_the_tools_runs_answers_and_merges() {
  let (mut served, repo, fake) = served();
  set_up(&mut served, &repo, &fake);
  let key = run(&mut served, "Escalate `git status`.");
  let approval = waiting_approval(&mut served);
  let id = approval["id"].as_str().expect("an id").to_owned();
  let decide = json!({ "project": "Kontur", "approval": id, "decision": "approve_once" });
  assert_eq!(
    served.refused("decide_approval", &decide)["code"],
    "automation_not_allowed"
  );
  let_automation_answer(&served);
  served.ok("decide_approval", &decide);
  served.until("the run to be ready for review", |served| {
    let task = served.ok("get_task", &json!({ "project": "Kontur", "key": key }));
    task["phase"] == "succeeded"
  });
  let changes = served.ok("get_changes", &json!({ "project": "Kontur", "run": "1" }));
  assert_eq!(changes["checkpointed"]["state"], "read", "{changes}");
  assert_eq!(changes["checkpointed"]["diff"]["files"][0]["path"], "README.md");
  let merged = served.ok("merge_run", &json!({ "project": "Kontur", "run": "1" }));
  assert_eq!(merged["merge"]["into"], "main", "{merged}");
  assert_eq!(merged["merge"]["kind"], "fast_forward");
  let readme = git(&repo, &["show", "main:README.md"]);
  assert_eq!(
    readme, "hi\nescalated\nhello\n",
    "main holds the agent's work, the approved command's included"
  );
  let again = served.refused("merge_run", &json!({ "project": "Kontur", "run": "1" }));
  assert_eq!(again["code"], "already_merged");
}

#[test]
fn domain_automation_cannot_approve_a_network_request() {
  let (mut served, repo, fake) = served();
  set_up(&mut served, &repo, &fake);
  run(&mut served, "Escalate `curl https://example.com`.");
  let approval = waiting_approval(&mut served);
  let_automation_answer(&served);
  let id = approval["id"].as_str().expect("an id").to_owned();
  let approve = served.refused(
    "decide_approval",
    &json!({ "project": "Kontur", "approval": id, "decision": "approve_once" }),
  );
  assert_eq!(approve["code"], "person_only", "{approve}");
  let still = served.ok("list_approvals", &json!({ "project": "Kontur" }));
  assert_eq!(still["approvals"][0]["id"], id, "the request still waits for a person");
  served.ok(
    "decide_approval",
    &json!({ "project": "Kontur", "approval": id, "decision": "deny" }),
  );
}
