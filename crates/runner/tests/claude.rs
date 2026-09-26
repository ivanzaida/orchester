//! Runs whose step runs on the Claude Code CLI, end to end against `orchester-fake-claude`, a
//! stand-in that prints stream-json (see `tests/support/fake_claude.rs`); the real CLI never runs in
//! tests. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test claude`.

mod support;

use std::time::Duration;

use orchester_core::{
  connection::{
    AgentProvider,
    stream::{AgentEvent, CommandStatus, FileChangeKind, InvocationOutcome, OutputStream},
  },
  id::{IdempotencyKey, RunId},
  resource::StepKey,
  run::{PauseReason, RaisedLimits, RunLimit, RunStatus, StepFailure, StepStatus},
  store::ActivityStore,
  tokens::{TokenBudget, TokensUsed},
};
use orchester_store::ProjectDb;

use self::support::{
  PATIENCE, Setup, TestProject, git, is_running, project, reported, scheduler, wait_for, wait_for_turns,
};

/// Counted tokens of one message of the fake CLI.
const MESSAGE: u64 = 12 + 9_700 + 27;

fn claude(brief: &'static str) -> TestProject {
  project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new(brief)
  })
}

fn events(db: &ProjectDb, run: RunId) -> Vec<AgentEvent> {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .map(|entry| entry.event)
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_runs_a_step_and_commits_its_changes() {
  let project = claude("Append one line.");
  let root = project.repo.path();
  let (scheduler, mut events_in) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let run = wait_for(root, &mut events_in, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let head = git(root, &["rev-parse", run.branch().as_str()]);
  assert_eq!(git(root, &["show", &format!("{head}:README.md")]), "hi\nhello");

  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, id, "argv: ").pop().expect("argv reported");
  assert!(
    argv.starts_with(
      "-p --output-format stream-json --verbose --model haiku --effort low --strict-mcp-config --setting-sources  \
       --disable-slash-commands --tools Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit,Bash --permission-mode \
       acceptEdits --disallowedTools WebFetch WebSearch --allowedTools Bash(git:*) --session-id "
    ),
    "{argv}"
  );
  let environment = reported(&db, id, "env: ").pop().expect("environment reported");
  assert!(
    environment.split(',').all(|name| !name.starts_with("CARGO")
      && !name.starts_with("RUST")
      && !name.starts_with("CLAUDE_CODE")
      && name != "CLAUDECODE"
      && !name.starts_with("ANTHROPIC")),
    "{environment}"
  );

  // The session is the id Orchester gave the new session, as the CLI reported it.
  let session = argv.rsplit(' ').next().expect("the session id").to_owned();
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert_eq!(invocation.session.as_deref(), Some(session.as_str()));
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Completed));
  assert_eq!(invocation.usage.counted(), 3 * MESSAGE);
  assert_eq!(invocation.usage.cached_input, 3 * 2_432);

  let events = events(&db, id);
  assert!(events.contains(&AgentEvent::CommandFinished {
    item: "toolu_git".into(),
    command: "git status --short".into(),
    output: " M README.md".into(),
    exit_code: None,
    status: CommandStatus::Completed,
  }));
  assert!(events.iter().any(|event| matches!(
    event,
    AgentEvent::FileChanged { changes, .. } if changes[0].kind == FileChangeKind::Modified
  )));
  assert!(events.contains(&AgentEvent::Raw {
    stream: OutputStream::Stdout,
    line: "a line that is not JSON".into()
  }));
  // The final message is the step result; its summary is the step's output.
  let outputs = db.step_outputs(id).expect("outputs");
  assert_eq!(outputs[0].text, "Finished Implement");
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_permission_denial_is_recorded_and_the_step_goes_on() {
  let project = claude("Ask for the web.");
  let root = project.repo.path();
  let (scheduler, mut events_in) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let run = wait_for(root, &mut events_in, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert!(events(&db, id).contains(&AgentEvent::PermissionDenied {
    item: "toolu_web".into(),
    command: "curl https://example.com".into(),
  }));
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_that_is_not_signed_in_fails_the_step_with_a_typed_outcome() {
  let project = claude("Please sign in.");
  let root = project.repo.path();
  let (scheduler, mut events_in) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let run = wait_for(root, &mut events_in, id, |run| run.status().is_terminal()).await;
  assert!(matches!(run.status(), RunStatus::Failed(_)), "{:?}", run.status());
  let implement = run
    .latest_execution(&StepKey::new("implement").expect("key"))
    .expect("the step ran");
  assert_eq!(implement.status(), StepStatus::Failed);
  assert_eq!(implement.failure(), Some(StepFailure::AgentFailed));
  let db = ProjectDb::open(root).expect("open");
  let message = "Not logged in · Please run /login".to_owned();
  assert_eq!(
    db.invocations(id).expect("invocations")[0].outcome,
    Some(InvocationOutcome::NotSignedIn {
      message: message.clone()
    })
  );
  assert!(events(&db, id).contains(&AgentEvent::NotSignedIn { message }));
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_stopped_at_a_limit_resumes_its_session_once_raised() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    budget: 5_000,
    ..Setup::new("Work in turns.")
  });
  let root = project.repo.path();
  let (scheduler, mut events_in) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let stopped = wait_for(root, &mut events_in, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(
    stopped.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens))
  );
  assert_eq!(stopped.used(), TokensUsed::new(MESSAGE));
  let db = ProjectDb::open(root).expect("open");
  let first = db.invocations(id).expect("invocations")[0].clone();
  assert_eq!(first.outcome, Some(InvocationOutcome::Terminated));
  let session = first.session.expect("the CLI reported its session");

  scheduler
    .raise_limit(
      id,
      RaisedLimits {
        budget: TokenBudget::new(2_000_000).expect("above zero"),
        max_duration: Duration::from_secs(30 * 60),
      },
    )
    .expect("the run continues");
  let done = wait_for(root, &mut events_in, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  let head = git(root, &["rev-parse", done.branch().as_str()]);
  assert_eq!(
    git(root, &["show", &format!("{head}:README.md")]),
    "hi\ndraft\ncontinued"
  );
  let argv = reported(&db, id, "argv: ");
  assert_eq!(argv.len(), 2);
  assert!(argv[1].ends_with(&format!("--resume {session}")), "{}", argv[1]);
  assert!(!argv[1].contains("--session-id"), "{}", argv[1]);
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_cancel_ends_the_agent() {
  let project = claude("Keep working.");
  let root = project.repo.path();
  let (scheduler, mut events_in) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  wait_for_turns(root, &mut events_in, id, 1).await;
  let db = ProjectDb::open(root).expect("open");
  let agent = reported(&db, id, "pid: ").pop().expect("the agent's pid");
  assert!(is_running(&agent));
  tokio::time::timeout(PATIENCE, scheduler.cancel_run(id))
    .await
    .expect("answered in time")
    .expect("the run is cancelled");
  assert_eq!(
    db.invocations(id).expect("invocations")[0].outcome,
    Some(InvocationOutcome::Terminated)
  );
  assert!(!is_running(&agent));
}
