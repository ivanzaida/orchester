//! A yielded Orchester MCP call must survive the model ending its turn before the person decides, and so must a
//! decision the yielded cell asks of Codex. The bounds of its continuations: `codex_tool_wait_bounds`.
mod support;

use std::{path::Path, time::Duration};

use orchester_core::{
  connection::stream::AgentEvent,
  id::RunId,
  run::{Answerer, ApprovalAnswer, InterruptCause, RunStatus},
  store::{ActivityStore, RunStore},
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;
use support::{
  PATIENCE, reported,
  tool_wait::{Waiting, command_setup, ended_turn, pending},
  wait_for, wait_for_idle,
};
use tokio::sync::mpsc;

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

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_a_call_codex_never_completes_is_continued_once() {
  // The continuation collects the cell; the call Codex still shows open earns no further turn.
  delayed_decision(ApprovalAnswer::Once, "Yield outside read. Never complete the call.").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_a_command_the_yielded_cell_asks_reaches_a_person() {
  let Waiting {
    project,
    runner,
    mut events,
    id,
    request,
  } = Waiting::begin(&command_setup("Yield outside read. Ask a command after.")).await;
  let root = project.repo.path();
  ended_turn(root, id).await;
  runner
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("decide the call");
  // Asked for the turn that started the cell, which ended before.
  let command = pending(root, &mut events, id, |request| request.command).await;
  assert_eq!(command.action, "cell-command");
  runner
    .decide_tool_approval(id, command.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("decide the command");
  collected_with_command(root, &mut events, id).await;
}

/// The run succeeded in one attempt and invocation, its one continuation collecting the cell after its command.
async fn collected_with_command(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, id: RunId) {
  let done = wait_for(root, events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(done.attempts().len(), 1);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.invocations(id).expect("invocations").len(), 1);
  assert_eq!(reported(&db, id, "turn-request: "), ["5", "7"]);
  assert_eq!(reported(&db, id, "cell command: "), ["accept"]);
  let results = reported(&db, id, "cell: ");
  assert_eq!(results.len(), 1);
  assert!(results[0].starts_with("approved: "), "{results:?}");
  assert!(!db.has_interrupted_tool_approval(id).expect("approvals"));
}

/// The cell's command waits as its turn ends: Codex is continued only once the person answered it, so the
/// continuation can neither end its turn on the waiting cell nor ask a second decision meanwhile.
async fn decision_waits_at_turn_end(brief: &'static str) {
  let mut waiting = Waiting::begin(&command_setup(brief)).await;
  waiting.approve_in_turn().await;
  let Waiting {
    project,
    runner,
    mut events,
    id,
    ..
  } = waiting;
  let root = project.repo.path();
  let command = pending(root, &mut events, id, |request| request.command).await;
  ended_turn(root, id).await;
  tokio::time::sleep(Duration::from_secs(1)).await;
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    reported(&db, id, "turn-request: "),
    ["5"],
    "no continuation while the decision waits"
  );
  assert_eq!(
    db.pending_tool_approvals(id).expect("pending"),
    std::slice::from_ref(&command)
  );
  runner
    .decide_tool_approval(id, command.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("decide the command");
  collected_with_command(root, &mut events, id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_a_waiting_decision_outlives_a_turn_that_would_give_up() {
  decision_waits_at_turn_end("Yield outside read. Ask a command after. Give up waiting.").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_a_waiting_decision_is_not_joined_by_another() {
  decision_waits_at_turn_end("Yield outside read. Ask a command after. Ask another decision.").await;
}
