//! Projects for the execution tests: a git repository with one commit, a signed-in Codex
//! connection running `orchester-fake-codex`, a published pipeline of agent steps and a queued
//! task, and a scheduler whose notifications the test reads.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, REASONING_EFFORT,
    SignIn, stream::AgentEvent,
  },
  id::{ConnectionId, IdempotencyKey, ProjectId, RunId, TaskId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  run::Run,
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_ipc::Event;
use orchester_runner::execution::{Notifier, Scheduler};
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::sync::mpsc;

pub const FAKE_CODEX: &str = env!("CARGO_BIN_EXE_orchester-fake-codex");
pub const PATIENCE: Duration = Duration::from_secs(30);

pub fn git(dir: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

/// What the test project runs.
pub struct Setup {
  /// The task's brief, which tells the fake CLI what to do.
  pub brief: &'static str,
  pub approval_gate: bool,
  /// `false`: one step, Implement; `true`: Implement, then Review.
  pub review: bool,
  pub budget: u64,
  pub max_duration: Duration,
}

impl Setup {
  pub fn new(brief: &'static str) -> Self {
    Self {
      brief,
      approval_gate: false,
      review: false,
      budget: 1_400_000,
      max_duration: Duration::from_secs(30 * 60),
    }
  }
}

pub struct TestProject {
  pub repo: TempDir,
  pub task: TaskId,
}

pub fn project(setup: &Setup) -> TestProject {
  let repo = tempfile::tempdir().expect("temporary directory");
  let root = repo.path();
  git(root, &["init", "--quiet", "--initial-branch=main"]);
  git(root, &["config", "user.name", "Orchester Test"]);
  git(root, &["config", "user.email", "test@orchester.invalid"]);
  git(root, &["config", "core.autocrlf", "false"]);
  git(root, &["config", "commit.gpgsign", "false"]);
  fs::write(root.join("README.md"), "hi\n").expect("write readme");
  git(root, &["add", "README.md"]);
  git(root, &["commit", "--quiet", "-m", "initial"]);
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(root, project).expect("create project");
  let connection = codex_connection();
  db.create_connection(&connection).expect("connection");
  let task = queued_task(&mut db, connection.id(), setup);
  TestProject { repo, task }
}

fn codex_connection() -> orchester_core::connection::Connection {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name("Codex · Kontur"),
    AgentProvider::CodexCli,
    PathBuf::from(FAKE_CODEX),
    Timestamp::now(),
  )
  .expect("absolute path");
  connection.record_check(ConnectionCheck {
    at: Timestamp::now(),
    result: CheckResult::Available {
      version: "0.139.0".into(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  });
  connection.into_connection()
}

fn agent_step(key: &str, title: &str, builder: orchester_core::id::ResourceId, setup: &Setup) -> Step {
  Step {
    key: StepKey::new(key).expect("key"),
    name: name(title),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
    },
    instructions: PromptText::new("Change README.md for {{task.id}}.").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: setup.approval_gate,
    on_failure: None,
    budget: None,
  }
}

fn queued_task(db: &mut ProjectDb, connection: ConnectionId, setup: &Setup) -> TaskId {
  let now = Timestamp::now();
  let settings =
    ProviderSettings::new(BTreeMap::from([(REASONING_EFFORT.to_owned(), "low".to_owned())])).expect("settings");
  let profile = AgentProfile::new(
    name("Builder"),
    "Implements tasks.".into(),
    PromptText::new("Work carefully.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("model"),
    settings,
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(builder, now).expect("publish profile");
  let mut steps = vec![agent_step("implement", "Implement", builder, setup)];
  if setup.review {
    let mut review = agent_step("review", "Review", builder, setup);
    review.depends_on = BTreeSet::from([StepKey::new("implement").expect("key")]);
    steps.push(review);
  }
  let pipeline = Pipeline::new(steps).expect("pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, now)
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(delivery, now).expect("publish pipeline");
  let budget = TokenBudget::new(setup.budget).expect("above zero");
  let details = TaskDetails {
    title: name("Say hello"),
    brief: setup.brief.into(),
    acceptance_criteria: vec!["README.md ends with hello".into()],
    pipeline: Some(delivery),
    limits: Some(TaskLimits::new(budget, setup.max_duration).expect("limits")),
  };
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], now)
    .expect("task")[0]
    .clone();
  task.queue().expect("complete task");
  db.save_task(&task).expect("save task");
  task.id()
}

pub fn scheduler(root: &Path) -> (Scheduler, mpsc::UnboundedReceiver<Event>) {
  let (sender, events) = mpsc::unbounded_channel();
  (Scheduler::new(root, Notifier::new(sender)), events)
}

/// The run once a notification shows it satisfying `done`.
pub async fn wait_for(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<Event>,
  run: RunId,
  done: impl Fn(&Run) -> bool,
) -> Run {
  let wait = async {
    loop {
      let current = ProjectDb::open(root)
        .expect("open")
        .run(run)
        .expect("read")
        .expect("run exists");
      if done(&current) {
        return current;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait)
    .await
    .expect("the run reaches the state in time")
}

/// Waits until the run's transcript holds `count` agent turns that started.
pub async fn wait_for_turns(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId, count: usize) {
  let db = ProjectDb::open(root).expect("open");
  let started = async {
    while db
      .activity(run, 0)
      .expect("activity")
      .iter()
      .filter(|entry| entry.event == AgentEvent::TurnStarted)
      .count()
      < count
    {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, started)
    .await
    .expect("the agent starts in time");
}

/// What the fake CLI reported on standard error after `prefix` (`"argv: "`, `"pid: "`), once
/// per invocation.
pub fn reported(db: &ProjectDb, run: RunId, prefix: &str) -> Vec<String> {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::Raw { line, .. } => line.strip_prefix(prefix).map(str::to_owned),
      _ => None,
    })
    .collect()
}

/// Whether process `pid` still runs.
pub fn is_running(pid: &str) -> bool {
  let output = if cfg!(windows) {
    Command::new("tasklist")
      .args(["/FI", &format!("PID eq {pid}"), "/NH"])
      .output()
  } else {
    Command::new("kill").args(["-0", pid]).output()
  }
  .expect("ask the system about the process");
  if cfg!(windows) {
    String::from_utf8_lossy(&output.stdout).contains(pid)
  } else {
    output.status.success()
  }
}
