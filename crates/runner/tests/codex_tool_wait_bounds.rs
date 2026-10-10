//! The continuations of a Codex turn whose Orchester calls outlive it are bounded: each needs a call returned or a
//! decision settled since the last, a failed turn gets none, at most three follow in a row without a decision or a
//! call the runner waited for, and a requested pause checkpoints instead of continuing.
mod support;

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome},
  id::IdempotencyKey,
  run::{InterruptCause, RunStatus},
  store::ActivityStore,
};
use orchester_store::ProjectDb;
use support::{
  PATIENCE, Setup, project, reported, scheduler_with,
  tool_wait::{Waiting, ended_turn},
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
