//! Runs executed end to end against `orchester-fake-codex`, a stand-in for the Codex CLI that
//! replays recorded output (see `tests/support/fake_codex.rs`); the real CLI never runs in tests.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test execution`.

use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, REASONING_EFFORT,
    SignIn,
    stream::{AgentEvent, CommandStatus, InvocationOutcome, OutputStream},
  },
  id::{ConnectionId, IdempotencyKey, ProjectId, RunId, TaskId},
  money::{Currency, Money},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  run::{InterruptCause, PreflightProblem, Run, RunStatus, StepStatus},
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::{Notifier, Scheduler};
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::sync::mpsc;

const FAKE_CODEX: &str = env!("CARGO_BIN_EXE_orchester-fake-codex");
const PATIENCE: Duration = Duration::from_secs(30);

fn git(dir: &Path, args: &[&str]) -> String {
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

/// A project repository with one commit, a signed-in Codex connection running the fake CLI, a
/// published one-step pipeline and a queued task whose brief is `brief`.
struct Project_ {
  repo: TempDir,
  task: TaskId,
}

fn project(brief: &str, approval_gate: bool) -> Project_ {
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
  let task = queued_task(&mut db, connection.id(), brief, approval_gate);
  Project_ { repo, task }
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

fn queued_task(db: &mut ProjectDb, connection: ConnectionId, brief: &str, approval_gate: bool) -> TaskId {
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
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
    },
    instructions: PromptText::new("Change README.md for {{task.id}}.").expect("prompt"),
    depends_on: Default::default(),
    approval_gate,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, now)
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(delivery, now).expect("publish pipeline");
  let details = TaskDetails {
    title: name("Say hello"),
    brief: brief.into(),
    acceptance_criteria: vec!["README.md ends with hello".into()],
    pipeline: Some(delivery),
    limits: Some(
      TaskLimits::new(Money::from_micros(5_000_000, Currency::USD), Duration::from_secs(1800)).expect("limits"),
    ),
  };
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], now)
    .expect("task")[0]
    .clone();
  task.queue().expect("complete task");
  db.save_task(&task).expect("save task");
  task.id()
}

fn scheduler(root: &Path) -> (Scheduler, mpsc::UnboundedReceiver<Event>) {
  let (sender, events) = mpsc::unbounded_channel();
  (Scheduler::new(root, Notifier::new(sender)), events)
}

/// The run once a notification shows it satisfying `done`.
async fn wait_for(
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

#[tokio::test(flavor = "multi_thread")]
async fn execution_runs_a_step_on_the_cli_and_commits_its_changes() {
  let project = project("Append one line.", false);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let key = IdempotencyKey::generate();
  let id = scheduler.start_run(key, project.task).expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.branch().as_str(), "orchester/orc-1-run-1");
  let checkpoint = run.last_checkpoint().expect("the step's changes were committed");
  let head = git(root, &["rev-parse", "orchester/orc-1-run-1"]);
  assert_eq!(checkpoint.revision().as_str(), head);
  assert_eq!(git(root, &["show", &format!("{head}:README.md")]), "hi\nhello");
  assert_eq!(git(root, &["log", "-1", "--format=%an", &head]), "Orchester Test");
  // The user's checkout is untouched.
  assert_eq!(fs::read_to_string(root.join("README.md")).expect("readme"), "hi\n");

  // Repeating the request with the same key returns the same run and starts nothing.
  assert_eq!(scheduler.start_run(key, project.task), Ok(id));
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.runs_of_task(project.task).expect("runs").len(), 1);

  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 1);
  let invocation = &invocations[0];
  assert_eq!(invocation.session.as_deref(), Some("fake-thread-1"));
  assert_eq!(invocation.usage.counted(), 12_166 - 2_432 + 5);
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Completed));
  assert!(invocation.ended_at.is_some());
  assert_transcript(&db, id);
}

fn assert_transcript(db: &ProjectDb, run: RunId) {
  let entries: Vec<AgentEvent> = db
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .map(|entry| entry.event)
    .collect();
  let stderr: Vec<&str> = entries
    .iter()
    .filter_map(|event| match event {
      AgentEvent::Raw {
        stream: OutputStream::Stderr,
        line,
      } => Some(line.as_str()),
      _ => None,
    })
    .collect();
  let argv = stderr
    .iter()
    .find_map(|line| line.strip_prefix("argv: "))
    .expect("argv reported");
  let expected = if cfg!(windows) {
    "-a never exec --json --ignore-user-config --ignore-rules -m gpt-5.5 -c model_reasoning_effort=low --sandbox \
     workspace-write -c windows.sandbox=elevated -C "
  } else {
    "-a never exec --json --ignore-user-config --ignore-rules -m gpt-5.5 -c model_reasoning_effort=low --sandbox \
     workspace-write -C "
  };
  assert!(argv.starts_with(expected), "{argv}");
  assert!(argv.ends_with(" -"), "{argv}");
  let environment = stderr
    .iter()
    .find_map(|line| line.strip_prefix("env: "))
    .expect("environment reported");
  assert!(
    environment
      .split(',')
      .all(|name| !name.starts_with("CARGO") && !name.starts_with("RUST")),
    "{environment}"
  );
  assert!(stderr.contains(&"prompt: # Your role: Builder"), "{stderr:?}");
  assert!(entries.contains(&AgentEvent::Message {
    text: "Appending the line.".into()
  }));
  assert!(entries.iter().any(|event| matches!(
    event,
    AgentEvent::CommandFinished {
      exit_code: Some(0),
      status: CommandStatus::Completed,
      ..
    }
  )));
  assert!(
    entries
      .iter()
      .any(|event| matches!(event, AgentEvent::FileChanged { .. }))
  );
  assert!(entries.contains(&AgentEvent::Raw {
    stream: OutputStream::Stdout,
    line: "a line that is not JSON".into()
  }));
  assert!(
    entries
      .iter()
      .any(|event| matches!(event, AgentEvent::TurnCompleted { .. }))
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn execution_fails_the_run_when_the_turn_fails() {
  let project = project("Please fail.", false);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert!(matches!(run.status(), RunStatus::Failed(_)));
  let db = ProjectDb::open(root).expect("open");
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert!(
    matches!(&invocation.outcome, Some(InvocationOutcome::TurnFailed { message }) if message.contains("not supported"))
  );
  assert!(run.last_checkpoint().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn execution_stops_the_agent_and_interrupts_the_run() {
  let project = project("Keep working.", false);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let db = ProjectDb::open(root).expect("open");
  let started = async {
    while db
      .activity(id, 0)
      .expect("activity")
      .iter()
      .all(|entry| entry.event != AgentEvent::TurnStarted)
    {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, started)
    .await
    .expect("the agent starts in time");
  assert_eq!(scheduler.active_runs(), [id]);
  tokio::time::timeout(PATIENCE, scheduler.stop_all(InterruptCause::QuitNow))
    .await
    .expect("stops in time");
  let run = db.run(id).expect("read").expect("run");
  assert_eq!(run.status(), &RunStatus::Interrupted(InterruptCause::QuitNow));
  let execution = &run.current_attempt().expect("attempt").executions()[0];
  assert_eq!(execution.status(), StepStatus::Running);
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Terminated));
}

#[tokio::test(flavor = "multi_thread")]
async fn execution_rejects_what_the_preflight_finds_and_tasks_that_are_not_queued() {
  let project = project("Append one line.", true);
  let root = project.repo.path();
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task),
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::ApprovalGate {
        step: StepKey::new("implement").expect("key")
      }]
    })
  );
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), TaskId::generate()),
    Err(Rejection::TaskNotFound)
  );
}
