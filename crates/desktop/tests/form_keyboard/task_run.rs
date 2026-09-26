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

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
};

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
/// pipeline: Implement, and with `review` also Review after it.
fn seed(repo: &Path, fake: &Path, review: bool) {
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
  let mut steps = vec![step.clone()];
  if review {
    steps.push(Step {
      key: StepKey::new("review").expect("key"),
      name: name("Review"),
      depends_on: [step.key].into(),
      ..step
    });
  }
  let pipeline = Pipeline::new(steps).expect("pipeline");
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

/// Whether the project's only task has a run that succeeded.
fn run_finished(repo: &Path) -> bool {
  latest_run(repo).is_some_and(|run| run.status() == &RunStatus::Succeeded)
}

fn has(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

/// The latest run of the project's only task, once it has one.
fn latest_run(repo: &Path) -> Option<orchester_core::run::Run> {
  let db = ProjectDb::open(repo).expect("open the project");
  let task = db.tasks().expect("tasks").into_iter().next()?;
  db.runs_of_task(task.id()).expect("runs").pop()
}

/// Fills the new task form with `brief` and `budget` and presses Create & run until the task
/// page opens.
fn create_and_run(desktop: &mut Desktop, brief: &str, budget: &str) {
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for("task-title");
  set_text(desktop, "task-title", "Say hello");
  set_text(desktop, "task-brief", brief);
  set_text(desktop, "criterion-0", "README.md ends with hello");
  set_text(desktop, "task-budget", budget);
  set_text(desktop, "task-time-limit", "30");
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
  wait_long(desktop, "the task page", |desktop| {
    // Pressing before the runner is online fails visibly; pressing again retries the same run.
    if !has(desktop, "task-page") && has(desktop, "create-and-run") {
      desktop.click("create-and-run");
    }
    has(desktop, "task-page")
  });
}

#[test]
fn create_and_run_opens_the_task_page_with_the_run_and_its_activity() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
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

#[test]
fn run_controls_raise_the_limit_of_a_run_stopped_at_it_and_continue() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Work in turns.", "5000");
  wait_long(&mut desktop, "the raise-limit panel", |desktop| {
    has(desktop, "raise-limit")
  });
  assert!(shows(
    &mut desktop,
    "task-notice",
    "Stopped at the token limit · checkpoint"
  ));
  assert!(shows(&mut desktop, "task-this-run", "9.7k / 5k tokens · limit reached"));
  assert!(has(&mut desktop, "task-cancel-run"));
  assert!(!has(&mut desktop, "task-pause-run"));
  // The token input is focused, offering half again the budget.
  assert_eq!(desktop.focused().as_deref(), Some("raise-limit-tokens"));
  assert_eq!(desktop.value("raise-limit-tokens"), "100,000");
  assert!(shows(&mut desktop, "raise-limit-continue", "Continue with 100k tokens"));
  desktop.click("raise-limit-continue");
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
  wait_long(&mut desktop, "the run controls to go", |desktop| {
    !has(desktop, "raise-limit") && !has(desktop, "task-cancel-run")
  });
}

#[test]
fn run_controls_cancel_a_working_run() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  wait_long(&mut desktop, "Pause run", |desktop| has(desktop, "task-pause-run"));
  desktop.click("task-cancel-run");
  wait_long(&mut desktop, "the run to be cancelled", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Cancelled)
  });
  wait_long(&mut desktop, "the cancelled run", |desktop| {
    has(desktop, "task-page") && !has(desktop, "task-cancel-run")
  });
  assert!(shows(&mut desktop, "task-notice", "The run was cancelled"));
}

#[test]
fn run_controls_pause_at_the_checkpoint_and_resume() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, true));
  create_and_run(&mut desktop, "Work slowly.", "1400000");
  wait_long(&mut desktop, "Pause run", |desktop| has(desktop, "task-pause-run"));
  desktop.click("task-pause-run");
  wait_long(&mut desktop, "Pausing", |desktop| {
    desktop
      .tree
      .get_element_by_id("task-pause-run")
      .is_some_and(|button| contains_text(button, "Pausing…"))
      || has(desktop, "task-resume-run")
  });
  wait_long(&mut desktop, "Resume run", |desktop| has(desktop, "task-resume-run"));
  assert!(shows(&mut desktop, "task-notice", "Paused at checkpoint"));
  let paused = latest_run(desktop.repo()).expect("run");
  assert!(matches!(paused.status(), RunStatus::Paused(_)));
  desktop.click("task-resume-run");
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
}
