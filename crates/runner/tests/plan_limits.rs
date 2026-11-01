//! Plan limits during runs, end to end against the fake CLIs: the readings the runner keeps, and when an account's
//! limit holds a step back or stops it. Run with `cargo test -p orchester-runner --features test-agent --test
//! plan_limits plan_limits`.

mod support;

use std::fs;

use orchester_core::{
  connection::{
    AgentProvider,
    stream::{AgentEvent, InvocationOutcome},
  },
  id::{IdempotencyKey, RunId},
  run::{PauseReason, Run, RunLimit, RunStatus},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{Setup, StepSpec};

#[tokio::test(flavor = "multi_thread")]
async fn plan_limits_runner_sends_both_cli_readings_and_keeps_them_in_the_history_not_the_transcript() {
  for provider in [AgentProvider::CodexCli, AgentProvider::ClaudeCodeCli] {
    let project = support::project(&support::Setup {
      provider,
      ..support::Setup::new(if provider == AgentProvider::CodexCli {
        "Report plan limits. Wait for release."
      } else {
        "Report plan limits"
      })
    });
    let root = project.repo.path();
    let (scheduler, mut events) = support::scheduler(root);
    let run = scheduler
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("start");
    let received = tokio::time::timeout(support::PATIENCE, async {
      loop {
        if let Some(Event::PlanLimits(reading)) = events.recv().await {
          break reading;
        }
      }
    })
    .await;
    if provider == AgentProvider::CodexCli {
      let live = ProjectDb::open(root)
        .expect("database")
        .run(run)
        .expect("run")
        .expect("exists");
      fs::write(root.join(".orchester/fake-release"), "release").expect("release the fake CLI");
      assert!(
        !live.status().is_terminal(),
        "App Server's notification is published while the turn still runs"
      );
    }
    let received = received.expect("live reading");
    assert_eq!(received.provider, provider);
    assert_eq!(received.limits.windows.len(), 1);
    let expected = if provider == AgentProvider::CodexCli {
      65.0
    } else {
      96.0
    };
    assert_eq!(received.limits.windows[0].percent(), Some(expected));
    support::wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
    let db = ProjectDb::open(root).expect("database");
    assert!(db.run(run).expect("run").expect("exists").status().is_terminal());
    assert!(db.activity(run, 0).expect("transcript").iter().all(|entry| {
      entry.event.name() != "plan_limits"
        && !matches!(&entry.event, AgentEvent::Raw { line, .. } if line.contains("rateLimits") || line.contains("rate_limit"))
    }));
    let history = db.limit_snapshots(Timestamp::from_unix_millis(0)).expect("history");
    assert_eq!(history.len(), 1, "the report is kept in the plan limits history");
    assert_eq!(history[0].provider, provider);
    assert_eq!(history[0].limits, received.limits);
    assert_eq!(history[0].at, received.at);
    let source = history[0].source.as_ref().expect("the run that reported it");
    assert_eq!(source.run, db.run(run).expect("run").expect("exists").number());
  }
}

/// A started Codex run on the project's Default account, its scheduler and its events.
type Started = (support::TestProject, Scheduler, mpsc::UnboundedReceiver<Event>, RunId);

/// A Codex run of Implement, whose agent does as `implement` says, then, with `review`, Review.
async fn started(implement: &'static str, review: bool) -> Started {
  let mut steps = vec![StepSpec::new("implement", "Implement", implement, &[])];
  if review {
    steps.push(StepSpec::new("review", "Review", "Review the change.", &["implement"]));
  }
  let project = support::project(&Setup {
    steps,
    ..Setup::new("Say hello.")
  });
  let (scheduler, events) = support::scheduler(project.repo.path());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  (project, scheduler, events, id)
}

fn stopped_at_plan_limit(run: &Run) -> bool {
  matches!(
    run.status(),
    RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { .. }))
  )
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_limits_usable_credits_carry_a_full_window_into_the_next_step() {
  let (project, _scheduler, mut events, id) = started("Spend credits past the window.", true).await;
  let root = project.repo.path();
  let run = support::wait_for(root, &mut events, id, |run| {
    run.status().is_terminal() || stopped_at_plan_limit(run)
  })
  .await;
  assert_eq!(
    run.status(),
    &RunStatus::Succeeded,
    "Review starts on credits though the five-hour window is used up and Codex said rate_limit_reached"
  );
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.invocations(id).expect("read").len(), 2);
  let reading = db.last_limit_reading(id).expect("read").expect("the mixed reading");
  assert!(
    reading.resets_at(Timestamp::now()).is_some(),
    "the window is still shown full"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_limits_a_full_window_without_credits_holds_the_next_step_back() {
  let (project, _scheduler, mut events, id) = started("Fill the window.", true).await;
  let root = project.repo.path();
  let run = support::wait_for(root, &mut events, id, |run| {
    run.status().is_terminal() || stopped_at_plan_limit(run)
  })
  .await;
  assert!(stopped_at_plan_limit(&run), "{:?}", run.status());
  let invocations = ProjectDb::open(root).expect("open").invocations(id).expect("read");
  assert_eq!(
    invocations.len(),
    1,
    "no agent started for Review while the limit holds"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_limits_a_turn_codex_failed_for_another_reason_is_no_plan_limit() {
  let (project, _scheduler, mut events, id) = started("Fail for another reason.", false).await;
  let root = project.repo.path();
  let run = support::wait_for(root, &mut events, id, |run| {
    run.attempts().len() > 1 || stopped_at_plan_limit(run)
  })
  .await;
  assert_eq!(
    run.attempts().len(),
    2,
    "the failed step restarts as any failed step does, though its window was full: {:?}",
    run.status()
  );
  let first = ProjectDb::open(root)
    .expect("open")
    .invocations(id)
    .expect("read")
    .remove(0);
  assert!(
    matches!(&first.outcome, Some(InvocationOutcome::TurnFailed { message }) if message.contains("stream disconnected")),
    "{:?}",
    first.outcome
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_limits_codex_refusing_at_its_usage_limit_stops_despite_credits() {
  let (project, _scheduler, mut events, id) = started("Refuse at the usage limit.", false).await;
  let root = project.repo.path();
  let run = support::wait_for(root, &mut events, id, |run| {
    run.status().is_terminal() || stopped_at_plan_limit(run)
  })
  .await;
  assert!(stopped_at_plan_limit(&run), "{:?}", run.status());
  assert_eq!(run.attempts().len(), 1, "a refusal is no step failure to restart");
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.invocations(id).expect("read").len(), 1);
  let reading = db.last_limit_reading(id).expect("read").expect("the reading");
  assert!(
    reading.reached_limit_resets().is_some(),
    "the run continues at the reset, once"
  );
}
