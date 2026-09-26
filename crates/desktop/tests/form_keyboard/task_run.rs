//! Create & run end to end, headlessly: the new task form queues the task and the real runner
//! runs its one step on `orchester-fake-codex`, a stand-in for the Codex CLI that replays
//! recorded output; then the task page shows the run and its transcript. The runner talks to the
//! desktop over a named pipe, which exists on Windows only.
#![cfg(windows)]

use std::{
  collections::BTreeMap,
  env, fs,
  path::{Path, PathBuf},
  process::Command,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  id::ConnectionId,
  name::Name,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  run::RunStatus,
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_desktop::app_paths::runner_beside;
use orchester_store::ProjectDb;

use crate::harness::Desktop;

/// A real run takes longer than the harness's default wait.
const PATIENCE: Duration = Duration::from_secs(60);

/// The runner and the fake CLI, built into this profile's directory beside the desktop.
fn programs() -> (PathBuf, PathBuf) {
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
  let test_exe = env::current_exe().expect("test executable path");
  let profile_dir = test_exe
    .parent()
    .and_then(Path::parent)
    .expect("test executables live in <target>/<profile>/deps");
  let fake = profile_dir
    .join("orchester-fake-codex")
    .with_extension(env::consts::EXE_EXTENSION);
  (runner_beside(&profile_dir.join("orchester.exe")), fake)
}

fn git(repo: &Path, args: &[&str]) {
  let status = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .status()
    .expect("run git");
  assert!(status.success(), "git {args:?}");
}

fn init_repository(repo: &Path) {
  git(repo, &["init", "--quiet", "--initial-branch=main"]);
  git(repo, &["config", "user.name", "Orchester Test"]);
  git(repo, &["config", "user.email", "test@orchester.invalid"]);
  git(repo, &["config", "commit.gpgsign", "false"]);
  fs::write(repo.join("README.md"), "hi\n").expect("write readme");
  git(repo, &["add", "README.md"]);
  git(repo, &["commit", "--quiet", "-m", "initial"]);
}

/// A signed-in Codex connection running `fake`, a Builder profile on it and a published
/// one-step pipeline.
fn seed(repo: &Path, fake: &Path) {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let now = Timestamp::now();
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex · Kontur").expect("name"),
    AgentProvider::CodexCli,
    fake.to_path_buf(),
    now,
  )
  .expect("absolute path");
  connection.record_check(ConnectionCheck {
    at: now,
    result: CheckResult::Available {
      version: "0.139.0".into(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  });
  let connection = connection.into_connection();
  db.create_connection(&connection).expect("connection");
  let profile = AgentProfile::new(
    Name::new("Builder").expect("role"),
    String::new(),
    PromptText::new("Work carefully.").expect("prompt"),
    connection.id(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::new(BTreeMap::new()).expect("settings"),
  );
  let name = |value: &str| Name::new(value).expect("name");
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(builder, now).expect("publish profile");
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
    },
    instructions: PromptText::new("Append one line.").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let id = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, now)
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, now).expect("publish pipeline");
}

fn set_text(desktop: &mut Desktop, id: &str, value: &str) {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("text input {id}"))
    .set_value(value);
  desktop.pass();
}

/// Lets the runner work until `done` holds.
fn wait_long(desktop: &mut Desktop, what: &str, done: impl Fn(&mut Desktop) -> bool) {
  let start = Instant::now();
  while start.elapsed() < PATIENCE {
    desktop.tree.tick_futures();
    desktop.pass();
    if done(desktop) {
      return;
    }
    thread::sleep(Duration::from_millis(20));
  }
  panic!("timed out waiting for {what}");
}

/// Whether the project's only task has a run that ended.
fn run_finished(repo: &Path) -> bool {
  let db = ProjectDb::open(repo).expect("open the project");
  let tasks = db.tasks().expect("tasks");
  let Some(task) = tasks.first() else {
    return false;
  };
  let runs = db.runs_of_task(task.id()).expect("runs");
  runs.last().is_some_and(|run| run.status() == &RunStatus::Succeeded)
}

fn has(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

#[test]
fn create_and_run_opens_the_task_page_with_the_run_and_its_activity() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake));
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for("task-title");
  set_text(&mut desktop, "task-title", "Say hello");
  set_text(&mut desktop, "task-brief", "Append one line to README.md.");
  set_text(&mut desktop, "criterion-0", "README.md ends with hello");
  set_text(&mut desktop, "task-budget", "5");
  set_text(&mut desktop, "task-time-limit", "30");
  desktop.settle("the published pipeline", |tree| {
    tree
      .get_element_by_id_mut("pipeline-selector")
      .and_then(|element| element.as_select())
      .is_some()
  });
  desktop
    .tree
    .get_element_by_id_mut("pipeline-selector")
    .and_then(|element| element.as_select())
    .expect("the pipeline select")
    .commit(1);
  desktop.pass();
  wait_long(
    &mut desktop,
    "Create & run to be enabled after the runner is online",
    |desktop| desktop.tree.get_element_by_id_mut("create-and-run").is_some(),
  );
  wait_long(&mut desktop, "the task page", |desktop| {
    // Pressing before the runner is online fails visibly; pressing again retries the same run.
    if !has(desktop, "task-page") && has(desktop, "create-and-run") {
      desktop.click("create-and-run");
    }
    has(desktop, "task-page")
  });
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
  desktop.click("task-tab-activity");
  wait_long(&mut desktop, "the transcript", |desktop| {
    has(desktop, "activity-step-header")
  });
  assert!(has(&mut desktop, "activity-invocation"));
  desktop.click("activity-step-header");
  wait_long(&mut desktop, "the step's entries", |desktop| {
    has(desktop, "activity-entry")
  });
  let head = Command::new("git")
    .arg("-C")
    .arg(desktop.repo())
    .args(["show", "orchester/kon-1-run-1:README.md"])
    .output()
    .expect("run git");
  assert_eq!(String::from_utf8_lossy(&head.stdout), "hi\nhello\n");
}
