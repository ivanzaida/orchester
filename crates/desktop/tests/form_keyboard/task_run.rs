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
  sync::OnceLock,
};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, StepPermissions},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  run::RunStatus,
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_desktop::app_paths::runner_beside;
use orchester_store::ProjectDb;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
};

/// The runner and the fake CLI, built into this profile's directory beside the desktop. They are
/// built once per test process, before any test starts them: the tests run side by side, and a
/// build could not replace a program another test is running (Windows keeps a running program's
/// file).
pub(crate) fn programs() -> (PathBuf, PathBuf) {
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

/// The stand-in for the Claude Code CLI, built by [`programs`] beside the fake Codex CLI.
pub(crate) fn fake_claude(fake_codex: &Path) -> PathBuf {
  fake_codex
    .with_file_name("orchester-fake-claude")
    .with_extension(env::consts::EXE_EXTENSION)
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

pub(crate) fn init_repository(repo: &Path) {
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
pub(crate) fn seed(repo: &Path, fake: &Path, review: bool) {
  seed_steps(repo, fake, |builder| {
    let mut steps = vec![agent_step(builder, "implement", "Implement", "Append one line.", &[])];
    if review {
      steps.push(agent_step(
        builder,
        "review",
        "Review",
        "Append one line.",
        &["implement"],
      ));
    }
    steps
  });
}

/// A write step on `builder` that runs after `after`, doing what `instructions` tell the fake CLI.
pub(crate) fn agent_step(builder: ResourceId, key: &str, name: &str, instructions: &str, after: &[&str]) -> Step {
  Step {
    key: StepKey::new(key).expect("key"),
    name: Name::new(name).expect("name"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new(instructions).expect("prompt"),
    depends_on: after.iter().map(|key| StepKey::new(key).expect("key")).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

/// A signed-in Codex connection running `fake`, a Builder profile on it and a published pipeline
/// of the steps `steps` makes for that profile.
pub(crate) fn seed_steps(repo: &Path, fake: &Path, steps: impl FnOnce(ResourceId) -> Vec<Step>) {
  seed_agent(repo, fake, AgentProvider::CodexCli, steps);
}

/// A signed-in connection to `provider`'s CLI running `fake`, a Builder profile on it and a
/// published pipeline of the steps `steps` makes for that profile.
pub(crate) fn seed_agent(
  repo: &Path,
  fake: &Path,
  provider: AgentProvider,
  steps: impl FnOnce(ResourceId) -> Vec<Step>,
) {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let now = Timestamp::now();
  let (title, version, model) = match provider {
    AgentProvider::CodexCli => ("Codex · Kontur", "0.139.0", "gpt-5.5"),
    AgentProvider::ClaudeCodeCli => ("Claude · Kontur", "2.1.89", "haiku"),
  };
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(title).expect("name"),
    provider,
    fake.to_path_buf(),
    now,
  )
  .expect("absolute path");
  connection.record_check(ConnectionCheck {
    at: now,
    result: CheckResult::Available {
      version: version.into(),
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
    ModelId::new(model).expect("model"),
    ProviderSettings::new(BTreeMap::new()).expect("settings"),
  );
  // Claude Code cannot confine any command to the worktree, only run listed ones.
  let profile = match provider {
    AgentProvider::CodexCli => profile,
    AgentProvider::ClaudeCodeCli => profile.with_permissions(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(
        AllowedCommands::new(vec![CommandPrefix::new("git").expect("prefix")]).expect("commands"),
      ),
      NetworkAccess::On,
    )),
  };
  let name = |value: &str| Name::new(value).expect("name");
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(builder, now).expect("publish profile");
  let pipeline = Pipeline::new(steps(builder)).expect("pipeline");
  let id = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, now)
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, now).expect("publish pipeline");
}

pub(crate) fn set_text(desktop: &mut Desktop, id: &str, value: &str) {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("text input {id}"))
    .set_value(value);
  desktop.pass();
}

/// Whether the project's only task has a run that succeeded.
pub(crate) fn run_finished(repo: &Path) -> bool {
  latest_run(repo).is_some_and(|run| run.status() == &RunStatus::Succeeded)
}

/// Waits until the run succeeded and the task page shows it (Ready for review on the Summary
/// tab). The page reads the run again only when the runner reports the change, so for a moment
/// it still shows the run at work after the database has it finished: a step of a run the page
/// shows at work starts open in the transcript, one of a finished run collapsed.
pub(crate) fn finished_on_page(desktop: &mut Desktop) {
  desktop.wait_until("the finished run on the page", |desktop| {
    run_finished(desktop.repo())
      && has(desktop, "task-ready-notice")
      && shows(desktop, "task-ready-notice", "Ready for your review")
  });
}

pub(crate) fn has(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

/// The latest run of the project's only task, once it has one.
pub(crate) fn latest_run(repo: &Path) -> Option<orchester_core::run::Run> {
  let db = ProjectDb::open(repo).expect("open the project");
  let task = db.tasks().expect("tasks").into_iter().next()?;
  let latest = db.runs_of_task(task.id()).expect("runs").pop()?;
  Some(latest.expect("the latest run can be read"))
}

/// Fills the new task form with `brief` and `budget` and presses Create & run until the task
/// page opens.
pub(crate) fn create_and_run(desktop: &mut Desktop, brief: &str, budget: &str) {
  fill_new_task(desktop, brief, budget);
  desktop.wait_until("the task page", |desktop| {
    // Pressing before the runner is online fails visibly; pressing again retries the same run.
    if !has(desktop, "task-page") && has(desktop, "create-and-run") {
      desktop.click("create-and-run");
    }
    has(desktop, "task-page")
  });
}

/// Opens New task and fills it with `brief` and `budget` on the published pipeline.
pub(crate) fn fill_new_task(desktop: &mut Desktop, brief: &str, budget: &str) {
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
}

#[test]
fn create_and_run_opens_the_task_page_with_the_run_and_its_activity() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
  // The page shows the finished run before the transcript opens, so its step starts collapsed.
  finished_on_page(&mut desktop);
  desktop.click("task-tab-activity");
  desktop.wait_until("the transcript", |desktop| has(desktop, "activity-step-header"));
  assert!(has(&mut desktop, "activity-invocation"));
  desktop.click("activity-step-header");
  desktop.wait_until("the step's entries", |desktop| has(desktop, "activity-entry"));
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
  desktop.wait_until("the raise-limit panel", |desktop| has(desktop, "raise-limit"));
  assert!(shows(
    &mut desktop,
    "task-notice",
    "Stopped at the token limit · checkpoint"
  ));
  // Token counts arrive when a turn ends, so the agent was stopped after its turn.
  assert!(shows(
    &mut desktop,
    "task-notice",
    "Builder was stopped after its turn and its work was committed as a checkpoint."
  ));
  assert!(shows(
    &mut desktop,
    "task-progress-steps",
    "Stopped at limit · attempt 1 · 9.7k tokens ·"
  ));
  assert!(shows(&mut desktop, "task-this-run", "9.7k / 5k tokens · limit reached"));
  assert!(has(&mut desktop, "task-cancel-run"));
  assert!(!has(&mut desktop, "task-pause-run"));
  // The token input is focused, offering half again the budget.
  assert_eq!(desktop.focused().as_deref(), Some("raise-limit-tokens"));
  assert_eq!(desktop.value("raise-limit-tokens"), "100,000");
  assert!(shows(&mut desktop, "raise-limit-continue", "Continue with 100k tokens"));
  desktop.click("raise-limit-continue");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  desktop.wait_until("the run controls to go", |desktop| {
    !has(desktop, "raise-limit") && !has(desktop, "task-cancel-run")
  });
}

#[test]
fn run_controls_raise_the_step_budget_of_a_run_stopped_at_it_and_continue() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      let implement = agent_step(builder, "implement", "Implement", "Work in turns.", &[]);
      vec![Step {
        budget: Some(TokenBudget::new(1_000).expect("above zero")),
        ..implement
      }]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the raise-limit panel", |desktop| has(desktop, "raise-limit"));
  assert!(shows(
    &mut desktop,
    "task-notice",
    "Stopped at the step budget of Implement · checkpoint"
  ));
  assert!(shows(&mut desktop, "raise-limit", "Raise the step budget to continue"));
  assert!(shows(
    &mut desktop,
    "task-progress-steps",
    "Stopped at limit · attempt 1 · 9.7k tokens ·"
  ));
  assert!(!shows(&mut desktop, "task-progress-steps", "Paused"));
  // The column scrolls with the raise panel (`ZVdRa`), Latest activity with it, below the rows.
  assert!(shows(
    &mut desktop,
    "task-progress-scroll",
    "Raise the step budget to continue"
  ));
  assert!(shows(&mut desktop, "task-progress-scroll", "Latest activity"));
  assert!(shows(
    &mut desktop,
    "raise-limit",
    "Used 9.7k of 1k · Implement stopped mid-step"
  ));
  // Only the step's budget is raised: the run's time limit is not offered.
  assert!(!has(&mut desktop, "raise-limit-time"));
  assert!(has(&mut desktop, "task-cancel-run"));
  assert!(!has(&mut desktop, "task-resume-run"));
  assert_eq!(desktop.focused().as_deref(), Some("raise-limit-tokens"));
  assert_eq!(desktop.value("raise-limit-tokens"), "100,000");
  set_text(&mut desktop, "raise-limit-tokens", "5000");
  assert!(shows(
    &mut desktop,
    "raise-limit",
    "Enter more than the 9.7k tokens Implement already used."
  ));
  set_text(&mut desktop, "raise-limit-tokens", "30000");
  assert!(shows(&mut desktop, "raise-limit", "= 30k tokens · 29k more than now"));
  assert!(shows(&mut desktop, "raise-limit-continue", "Continue with 30k tokens"));
  desktop.click("raise-limit-continue");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  let run = latest_run(desktop.repo()).expect("run");
  assert_eq!(run.attempts().len(), 1);
  assert_eq!(
    run.step_budget(&StepKey::new("implement").expect("key")),
    TokenBudget::new(30_000).ok()
  );
}

#[test]
fn run_controls_cancel_a_working_run() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("Pause run", |desktop| has(desktop, "task-pause-run"));
  desktop.click("task-cancel-run");
  desktop.wait_until("the run to be cancelled", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Cancelled)
  });
  desktop.wait_until("the cancelled run", |desktop| {
    has(desktop, "task-page") && !has(desktop, "task-cancel-run")
  });
  assert!(shows(&mut desktop, "task-notice", "The run was cancelled"));
}

#[test]
fn run_controls_pause_at_the_checkpoint_and_resume() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, true));
  create_and_run(&mut desktop, "Work slowly.", "1400000");
  desktop.wait_until("Pause run", |desktop| has(desktop, "task-pause-run"));
  desktop.click("task-pause-run");
  desktop.wait_until("Pausing", |desktop| {
    desktop
      .tree
      .get_element_by_id("task-pause-run")
      .is_some_and(|button| contains_text(button, "Pausing…"))
      || has(desktop, "task-resume-run")
  });
  desktop.wait_until("Resume run", |desktop| has(desktop, "task-resume-run"));
  assert!(shows(&mut desktop, "task-notice", "Paused at checkpoint"));
  let paused = latest_run(desktop.repo()).expect("run");
  assert!(matches!(paused.status(), RunStatus::Paused(_)));
  desktop.click("task-resume-run");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}
