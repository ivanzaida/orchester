//! The continuations of a Codex turn whose Orchester calls outlive it are bounded: each needs a call returned or a
//! decision settled since the last, a failed turn gets none, at most three follow in a row without a decision or a
//! call the runner waited for, and a requested pause checkpoints instead of continuing.
mod support;

use std::{path::Path, time::Duration};

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome},
  id::IdempotencyKey,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{Answerer, ApprovalAnswer, InterruptCause, PermissionsApply, RunStatus},
  store::{ActivityStore, RunStore},
};
use orchester_store::ProjectDb;
use support::{
  PATIENCE, Setup, project, reported, scheduler_with, threads,
  tool_wait::{Waiting, command_setup, ended_turn, pending},
  wait_for, wait_for_idle,
};

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_an_abandoned_call_that_never_returned_is_not_continued() {
  let project = project(&Setup::new("Abandon an Orchester call."));
  let root = project.repo.path();
  let (runner, mut events) = scheduler_with(root, project.vault.clone());
  let id = runner
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(done.attempts().len(), 1);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.invocations(id).expect("invocations").len(), 1);
  assert_eq!(reported(&db, id, "turn-request: "), ["5"]);
}

async fn wait_for_report(root: &Path, id: orchester_core::id::RunId, prefix: &str) {
  let db = ProjectDb::open(root).expect("open");
  tokio::time::timeout(PATIENCE, async {
    while reported(&db, id, prefix).is_empty() {
      tokio::time::sleep(Duration::from_millis(20)).await;
    }
  })
  .await
  .expect("the fake reaches its barrier");
}

fn release(root: &Path, id: orchester_core::id::RunId, marker: &str) {
  std::fs::write(
    root.join(".orchester/worktrees").join(id.to_string()).join(marker),
    "go",
  )
  .expect("release the result");
}

async fn command_finishes_after_turn(turn_before_decision: bool) {
  let brief = if turn_before_decision {
    "Yield outside read. Ask a command after. Hold command completion."
  } else {
    "Yield outside read. Ask a command after. Hold command completion. End turn after command approval."
  };
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
  if turn_before_decision {
    ended_turn(root, id).await;
  }
  runner
    .decide_tool_approval(id, command.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approve the command");
  wait_for_report(root, id, "cell-command-running: ").await;
  if !turn_before_decision {
    ended_turn(root, id).await;
  }
  tokio::time::sleep(Duration::from_millis(300)).await;
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    reported(&db, id, "turn-request: "),
    ["5"],
    "the command result still waits"
  );
  assert!(db.invocations(id).expect("invocations")[0].ended_at.is_none());
  release(root, id, "release-cell-command.txt");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(db.invocations(id).expect("invocations").len(), 1);
  assert_eq!(reported(&db, id, "turn-request: "), ["5", "7"]);
  assert_eq!(reported(&db, id, "cell command: "), ["accept"]);
  assert!(
    db.activity(id, 0)
      .expect("activity")
      .iter()
      .any(|entry| matches!(&entry.event, AgentEvent::CommandFinished { item, .. } if item == "cell-command"))
  );
  assert!(!db.has_interrupted_tool_approval(id).expect("approvals"));
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_approved_command_outlives_its_completed_turn() {
  command_finishes_after_turn(false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_command_decided_after_turn_finishes_before_continuation() {
  command_finishes_after_turn(true).await;
}

async fn permissions_wait_for_item(keep_for_next: bool) {
  let mut waiting = Waiting::start_with("Yield outside read. Hold MCP completion.").await;
  waiting.approve_in_turn().await;
  let Waiting {
    project,
    runner,
    mut events,
    id,
    ..
  } = waiting;
  let root = project.repo.path();
  let db = ProjectDb::open(root).expect("open");
  let run = db.run(id).expect("read").expect("run");
  let profile = run.profile_of(run.snapshot().steps()[0].key()).expect("profile");
  let permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  runner
    .change_permissions(id, profile, permissions, PermissionsApply::Now)
    .await
    .expect("apply now");
  ended_turn(root, id).await;
  tokio::time::sleep(Duration::from_millis(300)).await;
  assert_eq!(
    reported(&db, id, "turn-request: "),
    ["5"],
    "no turn under the old policy"
  );
  assert!(db.invocations(id).expect("invocations")[0].ended_at.is_none());
  if keep_for_next {
    runner
      .keep_permissions_for_next_invocation(id, profile)
      .await
      .expect("withdraw the restart");
    wait_for_report(root, id, "prompt: Collect the results of the Orchester tool calls").await;
  }
  release(root, id, "release-cell-item.txt");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  let invocations = db.invocations(id).expect("invocations");
  if keep_for_next {
    assert_eq!(invocations.len(), 1);
    assert_eq!(reported(&db, id, "turn-request: "), ["5", "7"]);
    assert!(done.changed_permissions(profile).expect("changed").applied().is_none());
    return;
  }
  assert_eq!(invocations.len(), 2);
  assert_eq!(invocations[0].session, invocations[1].session);
  assert_eq!(invocations[0].execution, invocations[1].execution);
  assert_eq!(reported(&db, id, "turn-request: "), ["5", "5"]);
  assert_eq!(threads(&db, id)[1]["sandbox"], "read-only");
  let applied = done
    .changed_permissions(profile)
    .expect("changed")
    .applied()
    .expect("applied");
  assert!(applied.checkpoint.is_some());
  assert!(!db.has_interrupted_tool_approval(id).expect("approvals"));
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_apply_now_waits_for_mcp_completion_before_a_new_turn() {
  permissions_wait_for_item(false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_withdrawing_apply_now_allows_the_waiting_continuation() {
  permissions_wait_for_item(true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_a_failed_turn_is_not_continued() {
  let mut waiting = Waiting::start_with("Yield outside read. Fail the turn.").await;
  // The call returned before the turn failed: a completed turn would be continued to collect it.
  waiting.approve_in_turn().await;
  let Waiting {
    project,
    runner,
    mut events,
    id,
    ..
  } = waiting;
  let root = project.repo.path();
  let db = ProjectDb::open(root).expect("open");
  ended_turn(root, id).await;
  let first = tokio::time::timeout(PATIENCE, async {
    loop {
      if let Some(ended) = db
        .invocations(id)
        .expect("invocations")
        .into_iter()
        .next()
        .filter(|invocation| invocation.ended_at.is_some())
      {
        return ended;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("the failed turn ends its invocation");
  assert!(
    matches!(&first.outcome, Some(InvocationOutcome::TurnFailed { message }) if message.contains("usage limit")),
    "{:?}",
    first.outcome
  );
  let turn_requests: Vec<_> = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .filter(|entry| entry.invocation == first.id)
    .filter_map(|entry| match entry.event {
      AgentEvent::Raw { line, .. } => line.strip_prefix("turn-request: ").map(str::to_owned),
      _ => None,
    })
    .collect();
  assert_eq!(turn_requests, ["5"], "no continuation after a failed turn");
  assert!(reported(&db, id, "cell: ").is_empty());
  runner.stop_all(InterruptCause::RunnerLost).await;
  wait_for_idle(&runner).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_a_command_outlives_the_continuation_that_collected_its_cell() {
  let Waiting {
    project,
    runner,
    mut events,
    id,
    request,
  } = Waiting::start_with("Yield outside read. Start command in continuation.").await;
  let root = project.repo.path();
  ended_turn(root, id).await;
  runner
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approve the call");
  wait_for_report(root, id, "cell-command-running: ").await;
  let db = ProjectDb::open(root).expect("open");
  tokio::time::timeout(PATIENCE, async {
    while db
      .activity(id, 0)
      .expect("activity")
      .iter()
      .filter(|entry| matches!(entry.event, AgentEvent::TurnCompleted { .. }))
      .count()
      < 2
    {
      tokio::time::sleep(Duration::from_millis(20)).await;
    }
  })
  .await
  .expect("the continuation ends before the command");
  tokio::time::sleep(Duration::from_millis(300)).await;
  assert!(db.invocations(id).expect("invocations")[0].ended_at.is_none());
  assert_eq!(reported(&db, id, "turn-request: "), ["5", "7"]);
  release(root, id, "release-cell-command.txt");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(db.invocations(id).expect("invocations").len(), 1);
  assert_eq!(reported(&db, id, "turn-request: "), ["5", "7", "8"]);
  assert!(
    db.activity(id, 0)
      .expect("activity")
      .iter()
      .any(|entry| matches!(&entry.event, AgentEvent::CommandFinished { item, .. } if item == "cell-command"))
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_continuations_without_a_decision_stop_after_three() {
  // Each turn calls a tool, so each has a returned call, and an abandoned call stays open: only the cap ends it.
  let project = project(&Setup::new("Abandon an Orchester call. Call a tool every turn."));
  let root = project.repo.path();
  let (runner, mut events) = scheduler_with(root, project.vault.clone());
  let id = runner
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(done.attempts().len(), 1);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.invocations(id).expect("invocations").len(), 1);
  assert_eq!(reported(&db, id, "turn-request: "), ["5", "7", "8", "9"]);
  assert_eq!(reported(&db, id, "every-turn call: "), ["answered"; 4]);
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_tool_wait_a_pause_requested_while_a_call_ran_checkpoints_instead_of_continuing() {
  let mut waiting = Waiting::start().await;
  waiting.approve_in_turn().await;
  let Waiting {
    project,
    runner,
    mut events,
    id,
    ..
  } = waiting;
  let root = project.repo.path();
  // Nothing waits for a decision now: the pause waits for the agent.
  runner.pause_run(id).await.expect("pause");
  ended_turn(root, id).await;
  let paused = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  wait_for_idle(&runner).await;
  assert_eq!(paused.attempts().len(), 1);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    reported(&db, id, "turn-request: "),
    ["5"],
    "no turn started only to be ended"
  );
  assert!(
    !db
      .activity(id, 0)
      .expect("activity")
      .iter()
      .any(|entry| matches!(entry.event, AgentEvent::TurnFailed { .. }))
  );
}
