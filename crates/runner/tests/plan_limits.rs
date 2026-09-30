use std::fs;
mod support;

use orchester_core::{connection::plan_limits::LimitState, time::Timestamp};
use orchester_runner::codex_limits::SessionLimits;

const SESSION: &str = "019994f0-1234-7000-8000-000000000000";
const CODEX: &str = include_str!("../../core/tests/fixtures/codex/plan-limits.jsonl");

#[tokio::test]
async fn plan_limits_codex_captured_payload_reaches_the_session_reader() {
  let folder = tempfile::tempdir().expect("home");
  let sessions = folder.path().join("sessions/2026/09/30");
  fs::create_dir_all(&sessions).expect("sessions");
  let file = sessions.join(format!("rollout-2026-09-30-{SESSION}.jsonl"));
  fs::write(
    file,
    include_str!("../../core/tests/fixtures/codex/captured-plan-limits.jsonl"),
  )
  .expect("capture");
  let mut reader = SessionLimits::new(folder.path().to_path_buf(), Timestamp::from_unix_millis(0));
  let readings = reader.read(SESSION).await.expect("read");
  assert_eq!(readings[0].limits.windows[0].percent(), 82.0);
  assert_eq!(
    readings[0].limits.credits.as_ref().expect("credits").balance.as_deref(),
    Some("62500")
  );
}

#[tokio::test]
async fn plan_limits_codex_session_only_new_complete_events() {
  let folder = tempfile::tempdir().expect("home");
  let sessions = folder.path().join("sessions/2026/09/29");
  fs::create_dir_all(&sessions).expect("sessions");
  let file = sessions.join(format!("rollout-2026-09-29-{SESSION}.jsonl"));
  let since = Timestamp::parse_utc("2026-09-29T11:25:00Z").expect("time");
  let old = CODEX.replace("11:26:00", "11:24:00");
  fs::write(&file, format!("{old}{CODEX}")).expect("session log");
  let mut reader = SessionLimits::new(folder.path().to_path_buf(), since);
  let readings = reader.read(SESSION).await.expect("read");
  assert_eq!(readings.len(), 1, "earlier session turns are not replayed");
  assert_eq!(
    readings[0].at,
    Timestamp::parse_utc("2026-09-29T11:26:00Z").expect("time")
  );
  assert_eq!(readings[0].limits.windows[0].state(since), LimitState::Normal);
  assert!(reader.read(SESSION).await.expect("read again").is_empty());
  fs::write(&file, format!("{old}{CODEX}{}", CODEX.trim_end())).expect("partial");
  assert!(reader.read(SESSION).await.expect("partial line").is_empty());
  fs::write(&file, format!("{old}{CODEX}{CODEX}")).expect("completed");
  assert_eq!(reader.read(SESSION).await.expect("new complete line").len(), 1);
  assert!(reader.read("../other").await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_limits_runner_sends_both_cli_readings_without_storing_them() {
  use orchester_core::{
    connection::AgentProvider,
    id::IdempotencyKey,
    store::{ActivityStore, RunStore},
  };
  use orchester_ipc::Event;
  use orchester_store::ProjectDb;
  for provider in AgentProvider::ALL {
    let project = support::project(&support::Setup {
      provider,
      ..support::Setup::new("Report plan limits")
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
    .await
    .expect("live reading");
    assert_eq!(received.provider, provider);
    assert_eq!(received.limits.windows.len(), 1);
    support::wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
    let db = ProjectDb::open(root).expect("database");
    assert!(db.run(run).expect("run").expect("exists").status().is_terminal());
    assert!(
      db.activity(run, 0)
        .expect("transcript")
        .iter()
        .all(|entry| entry.event.name() != "plan_limits")
    );
  }
}
