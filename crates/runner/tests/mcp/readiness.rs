//! The turn is withheld until Codex's required thread connection becomes ready.
use orchester_core::{
  id::RunId,
  run::{PauseReason, RunLimit, StuckDecision},
  store::RunStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use tokio::sync::mpsc;

use super::*;

fn setup(model: &'static str, brief: &'static str) -> Setup {
  let mut permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  permissions.set_interactive_tool_approvals(true);
  Setup {
    model: Some(model),
    mcp: vec![server(
      "readiness",
      stdio(TEST_COMMAND, None),
      McpEnvironment::default(),
    )],
    permissions: Some(permissions),
    ..Setup::new(brief)
  }
}

async fn start(setup: &Setup) -> (TestProject, Scheduler, mpsc::UnboundedReceiver<Event>, RunId) {
  let project = project(setup);
  let (scheduler, events) = scheduler_with(project.repo.path(), project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  (project, scheduler, events, id)
}

fn assert_ready_turns(db: &ProjectDb, id: RunId, turns: usize) {
  let states = reported(db, id, "mcp-readiness: ");
  assert_eq!(states, ["starting", "connected"].repeat(turns));
  assert_eq!(reported(db, id, "turn-request: ").len(), turns);
  assert!(left_out(db, &db.run(id).expect("read").expect("run")).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_readiness_starting_connects_before_a_fresh_turn() {
  for model in ["mcp-ready", "mcp-ready-orchester"] {
    let project = project(&setup(model, "Say hello."));
    let run = run_to_the_end(&project).await;
    assert_eq!(run.status(), &RunStatus::Succeeded);
    assert_ready_turns(&ProjectDb::open(project.repo.path()).expect("db"), run.id(), 1);
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_readiness_terminal_failure_and_missing_never_start_a_turn() {
  for (model, cause) in [
    (
      "mcp-ready-failed",
      "failed: startup refused token <redacted> second line",
    ),
    ("mcp-ready-missing", "did not load the MCP server `readiness`"),
  ] {
    let project = project(&setup(model, "Say hello."));
    let run = run_to_the_end(&project).await;
    assert!(matches!(run.status(), RunStatus::WaitingForPerson(_)));
    let db = ProjectDb::open(project.repo.path()).expect("db");
    assert!(reported(&db, run.id(), "turn-request: ").is_empty());
    let failures: Vec<_> = db
      .activity(run.id(), 0)
      .expect("activity")
      .into_iter()
      .filter_map(|entry| match entry.event {
        AgentEvent::TurnFailed { message } => Some(message),
        _ => None,
      })
      .collect();
    assert!(!failures.is_empty());
    assert!(failures.iter().all(|message| message.contains(cause)), "{failures:?}");
  }
}

async fn wait_starting(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, id: RunId) {
  tokio::time::timeout(PATIENCE, async {
    loop {
      if reported(&ProjectDb::open(root).expect("db"), id, "mcp-readiness: ").len() >= 2 {
        break;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("starting reported");
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_readiness_cancellation_interrupts_starting_and_an_unanswered_status() {
  for model in ["mcp-ready-forever", "mcp-ready-silent"] {
    let (project, scheduler, mut events, id) = start(&setup(model, "Say hello.")).await;
    let root = project.repo.path();
    wait_starting(root, &mut events, id).await;
    tokio::time::timeout(Duration::from_secs(5), scheduler.cancel_run(id))
      .await
      .expect("prompt cancellation")
      .expect("cancel");
    let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
    assert_eq!(run.status(), &RunStatus::Cancelled);
    assert!(reported(&ProjectDb::open(root).expect("db"), id, "turn-request: ").is_empty());
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_readiness_starting_and_unanswered_status_have_a_fixed_timeout() {
  for model in ["mcp-ready-forever", "mcp-ready-silent"] {
    let (project, scheduler, mut events, id) = start(&setup(model, "Say hello.")).await;
    let root = project.repo.path();
    tokio::time::timeout(Duration::from_secs(75), async {
      loop {
        let db = ProjectDb::open(root).expect("db");
        if db.activity(id, 0).expect("activity").iter().any(|entry|
          matches!(&entry.event, AgentEvent::TurnFailed { message } if message.contains("Timed out waiting") && message.contains("`readiness`") && message.ends_with("starting"))) { break; }
        events.recv().await.expect("notification");
      }
    }).await.expect("bounded readiness");
    scheduler.cancel_run(id).await.expect("stop any automatic retry");
    wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
    assert!(reported(&ProjectDb::open(root).expect("db"), id, "turn-request: ").is_empty());
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_readiness_is_repeated_on_automatic_and_manual_retries() {
  let (project, scheduler, mut events, id) = start(&setup("mcp-ready", "Please fail.")).await;
  let root = project.repo.path();
  wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::WaitingForPerson(_))
  })
  .await;
  support::wait_for_idle(&scheduler).await;
  scheduler.decide_stuck(id, StuckDecision::Retry).await.expect("retry");
  wait_for(root, &mut events, id, |run| {
    run.attempts().len() == 3 && matches!(run.status(), RunStatus::WaitingForPerson(_))
  })
  .await;
  support::wait_for_idle(&scheduler).await;
  assert_ready_turns(&ProjectDb::open(root).expect("db"), id, 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_readiness_is_repeated_on_quota_resume_of_the_same_thread() {
  let (project, scheduler, mut events, id) = start(&setup("mcp-ready", "Refuse at the usage limit.")).await;
  let root = project.repo.path();
  wait_for(root, &mut events, id, |run| {
    matches!(
      run.status(),
      RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { .. }))
    )
  })
  .await;
  support::wait_for_idle(&scheduler).await;
  scheduler.resume_run(id).await.expect("resume");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("db");
  assert_ready_turns(&db, id, 2);
  let threads = threads(&db, id);
  assert_eq!(threads[1]["threadId"], "fake-thread-1");
}
