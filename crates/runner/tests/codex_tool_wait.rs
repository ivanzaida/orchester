//! A yielded Orchester MCP call must survive the model ending its turn before the person decides.
mod support;

use std::{path::Path, time::Duration};

use orchester_core::{
  connection::stream::AgentEvent,
  id::{IdempotencyKey, RunId},
  run::{Answerer, ApprovalAnswer, InterruptCause, PendingToolApproval, RunStatus},
  store::{ActivityStore, RunStore},
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use support::{PATIENCE, Setup, TestProject, project, reported, scheduler_with, wait_for, wait_for_idle};
use tokio::sync::mpsc;

struct Waiting {
  project: TestProject,
  runner: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
  request: PendingToolApproval,
}

impl Waiting {
  async fn start() -> Self {
    Self::start_with("Yield outside read.").await
  }

  async fn start_with(brief: &'static str) -> Self {
    let project = project(&Setup::new(brief));
    let root = project.repo.path();
    let folder = root.join("deps-a");
    std::fs::create_dir(&folder).expect("outside folder");
    std::fs::write(folder.join("notes.txt"), "outside line").expect("outside file");
    let (runner, mut events) = scheduler_with(root, project.vault.clone());
    let id = runner
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("start");
    let request = pending(root, &mut events, id).await;
    Self {
      project,
      runner,
      events,
      id,
      request,
    }
  }
}

async fn pending(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, id: RunId) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      if let Some(request) = ProjectDb::open(root)
        .expect("open")
        .pending_tool_approvals(id)
        .expect("pending")
        .first()
      {
        return request.clone();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("approval arrives")
}

async fn ended_turn(root: &Path, id: RunId) {
  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  std::fs::write(worktree.join("yield-turn.txt"), "end the model turn").expect("release turn");
  tokio::time::timeout(PATIENCE, async {
    loop {
      if ProjectDb::open(root)
        .expect("open")
        .activity(id, 0)
        .expect("activity")
        .iter()
        .any(|entry| matches!(entry.event, AgentEvent::TurnCompleted { .. }))
      {
        return;
      }
      tokio::time::sleep(Duration::from_millis(20)).await;
    }
  })
  .await
  .expect("the model ends the turn");
}

async fn delayed_decision(answer: ApprovalAnswer, brief: &'static str) {
  let Waiting {
    project,
    runner: scheduler,
    mut events,
    id,
    request,
  } = Waiting::start_with(brief).await;
  let root = project.repo.path();
  assert_eq!(request.action, "search_external_file");
  ended_turn(root, id).await;
  tokio::time::sleep(Duration::from_secs(3)).await;
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    db.pending_tool_approvals(id).expect("pending"),
    std::slice::from_ref(&request)
  );
  let invocation = db.invocations(id).expect("invocations");
  assert_eq!(invocation.len(), 1);
  assert!(invocation[0].ended_at.is_none());
  assert!(invocation[0].timing.waiting_since().is_some());
  let approves = answer == ApprovalAnswer::Once;
  scheduler
    .decide_tool_approval(id, request.id, answer, Answerer::Person)
    .await
    .expect("decision delivered");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(done.attempts().len(), 1, "the continuation is in the same attempt");
  assert_eq!(
    db.invocations(id).expect("invocations").len(),
    1,
    "the same process collects its cell"
  );
  assert_eq!(reported(&db, id, "turn-request: "), ["5", "7"]);
  let turns = reported(&db, id, "turn: ");
  assert_eq!(turns.len(), 2);
  assert_eq!(turns[0], turns[1], "the continuation keeps the invocation's settings");
  let results = reported(&db, id, "cell: ");
  assert_eq!(results.len(), 1);
  assert!(
    results[0].starts_with(if approves { "approved: " } else { "denied: " }),
    "{results:?}"
  );
  if approves {
    assert!(results[0].contains("notes.txt"), "{results:?}");
  }
  assert!(!db.has_interrupted_tool_approval(id).expect("approvals"));
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_approval_after_a_completed_turn_continues_the_same_attempt() {
  delayed_decision(ApprovalAnswer::Once, "Yield outside read.").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_denial_after_a_completed_turn_continues_the_same_attempt() {
  delayed_decision(ApprovalAnswer::Deny, "Yield outside read.").await;
}

async fn result_before_turn_completion(answer: ApprovalAnswer) {
  let Waiting {
    project,
    runner,
    mut events,
    id,
    request,
  } = Waiting::start().await;
  let root = project.repo.path();
  let approves = answer == ApprovalAnswer::Once;
  runner
    .decide_tool_approval(id, request.id, answer, Answerer::Person)
    .await
    .expect("decide");
  let db = ProjectDb::open(root).expect("open");
  // Hold turn/completed until the runner has received the MCP item's completion and the result.
  tokio::time::timeout(PATIENCE, async {
    loop {
      if db
        .activity(id, 0)
        .expect("activity")
        .iter()
        .any(|entry| matches!(&entry.event, AgentEvent::McpToolFinished { item, .. } if item == "cell-search"))
        && !reported(&db, id, "cell-returned: ").is_empty()
      {
        break;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("the relay result and MCP completion arrive");
  assert!(
    !db
      .activity(id, 0)
      .expect("activity")
      .iter()
      .any(|entry| matches!(entry.event, AgentEvent::TurnCompleted { .. }))
  );
  assert!(
    reported(&db, id, "cell: ").is_empty(),
    "the model has not collected its cell"
  );
  ended_turn(root, id).await;
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(done.attempts().len(), 1);
  assert_eq!(db.invocations(id).expect("invocations").len(), 1);
  assert_eq!(reported(&db, id, "turn-request: "), ["5", "7"]);
  let results = reported(&db, id, "cell: ");
  assert_eq!(results.len(), 1);
  if approves {
    assert!(results[0].starts_with("approved: "), "{results:?}");
    assert!(results[0].contains("notes.txt"), "{results:?}");
  } else {
    assert!(results[0].starts_with("denied: "), "{results:?}");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_result_returned_before_turn_completion_is_collected() {
  result_before_turn_completion(ApprovalAnswer::Once).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_denial_returned_before_turn_completion_is_collected() {
  result_before_turn_completion(ApprovalAnswer::Deny).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_late_start_reply_does_not_confuse_the_continuation() {
  delayed_decision(ApprovalAnswer::Once, "Yield outside read. Delay turn reply.").await;
}

enum StopWaiting {
  Pause,
  Cancel,
  RunnerStop,
  Shutdown,
}

async fn interrupted_decision(stop: StopWaiting) {
  let Waiting {
    project,
    runner,
    mut events,
    id,
    request,
  } = Waiting::start().await;
  let root = project.repo.path();
  ended_turn(root, id).await;
  match stop {
    StopWaiting::Pause => {
      runner.pause_run(id).await.expect("pause");
      wait_for(root, &mut events, id, |run| {
        matches!(run.status(), RunStatus::Paused(_))
      })
      .await;
    }
    StopWaiting::Cancel => runner.cancel_run(id).await.expect("cancel"),
    StopWaiting::RunnerStop => runner.stop_all(InterruptCause::RunnerLost).await,
    StopWaiting::Shutdown => runner.finish_at_checkpoints().await,
  }
  wait_for_idle(&runner).await;
  let db = ProjectDb::open(root).expect("open");
  let run = db.run(id).expect("read").expect("run");
  assert_eq!(run.attempts().len(), 1, "a stopped decision consumes no failed attempt");
  assert!(!matches!(
    run.status(),
    RunStatus::Running | RunStatus::WaitingForPerson(_) | RunStatus::Failed(_)
  ));
  assert_eq!(db.invocations(id).expect("invocations").len(), 1);
  assert!(db.has_interrupted_tool_approval(id).expect("interrupted"));
  assert!(db.pending_tool_approvals(id).expect("pending").is_empty());
  assert!(reported(&db, id, "cell: ").is_empty(), "no unapproved outside read ran");
  assert!(
    !db
      .activity(id, 0)
      .expect("activity")
      .iter()
      .any(|entry| matches!(entry.event, AgentEvent::TurnFailed { .. }))
  );
  assert_eq!(
    runner
      .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
      .await,
    Err(Rejection::ToolApprovalNotAwaited),
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_pause_does_not_fail_the_attempt() {
  interrupted_decision(StopWaiting::Pause).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_cancel_does_not_fail_the_attempt() {
  interrupted_decision(StopWaiting::Cancel).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_runner_stop_does_not_fail_the_attempt() {
  interrupted_decision(StopWaiting::RunnerStop).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_shutdown_does_not_fail_the_attempt() {
  interrupted_decision(StopWaiting::Shutdown).await;
}
