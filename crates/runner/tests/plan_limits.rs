mod support;

use std::fs;

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::IdempotencyKey,
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;

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
