//! Runs executed end to end against `orchester-fake-codex`, a stand-in for the Codex CLI that
//! replays recorded output (see `tests/support/fake_codex.rs`); the real CLI never runs in tests.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test execution`.

mod support;

use std::fs;

use orchester_core::{
  connection::{
    AgentConnection, CatalogSource, CheckResult, ConnectionCheck, ConnectionStatus, ModelCatalog, SignIn,
    stream::{AgentEvent, CommandStatus, InvocationOutcome, OutputStream},
  },
  id::{IdempotencyKey, RunId, TaskId},
  resource::StepKey,
  run::{InterruptCause, PreflightProblem, RunStatus, StepStatus},
  store::{ActivityStore, ConnectionStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use self::support::{PATIENCE, Setup, git, project, scheduler, wait_for, wait_for_turns};

#[tokio::test(flavor = "multi_thread")]
async fn execution_runs_a_step_on_the_cli_and_commits_its_changes() {
  let project = project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let key = IdempotencyKey::generate();
  let id = scheduler.start_run(key, project.task).await.expect("the run starts");
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
  assert_eq!(scheduler.start_run(key, project.task).await, Ok(id));
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
  // The profile has network off, which Codex cannot enforce on Windows: network stays on there.
  let expected = if cfg!(windows) {
    "-a never exec --json --ignore-user-config --ignore-rules -m gpt-5.5 -c model_reasoning_effort=low --sandbox \
     workspace-write -c windows.sandbox=elevated -c sandbox_workspace_write.network_access=true --output-schema "
  } else {
    "-a never exec --json --ignore-user-config --ignore-rules -m gpt-5.5 -c model_reasoning_effort=low --sandbox \
     workspace-write --output-schema "
  };
  assert!(argv.starts_with(expected), "{argv}");
  // The step result schema is read from next to the project database, outside the worktree.
  assert!(argv.contains("step-result.schema.json -C "), "{argv}");
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
  let project = project(&Setup::new("Please fail."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
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
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let db = ProjectDb::open(root).expect("open");
  wait_for_turns(root, &mut events, id, 1).await;
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
  let project = project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let stored = db.connections().expect("connections").remove(0);
  let mut connection = AgentConnection::try_from(stored).expect("an agent connection");
  connection.record_check(ConnectionCheck {
    at: Timestamp::now(),
    result: CheckResult::Available {
      version: "0.139.0".into(),
      sign_in: SignIn::SignedOut,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  });
  db.save_connection(&connection.into_connection()).expect("save");
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::ConnectionNotUsable {
        step: StepKey::new("implement").expect("key"),
        status: ConnectionStatus::SignInRequired,
      }]
    })
  );
  assert_eq!(
    scheduler
      .start_run(IdempotencyKey::generate(), TaskId::generate())
      .await,
    Err(Rejection::TaskNotFound)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn execution_records_the_branch_the_checkout_has_when_the_run_starts() {
  let on_main = project(&Setup::new("Append one line."));
  let run = run_to_the_end(on_main.repo.path(), on_main.task).await;
  assert_eq!(run.base_branch().map(|branch| branch.as_str()), Some("main"));
  assert_eq!(run.base().as_str(), git(on_main.repo.path(), &["rev-parse", "main"]));

  let detached = project(&Setup::new("Append one line."));
  git(detached.repo.path(), &["switch", "--quiet", "--detach", "main"]);
  let run = run_to_the_end(detached.repo.path(), detached.task).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.base_branch(), None, "a detached HEAD has no branch to merge into");
}

async fn run_to_the_end(root: &std::path::Path, task: TaskId) -> orchester_core::run::Run {
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await
}
