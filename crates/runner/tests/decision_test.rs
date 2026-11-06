#[path = "../../store/tests/support/mod.rs"]
mod seeded;
#[path = "decision_test/server.rs"]
mod server;
#[path = "support/vault.rs"]
mod vault;

use std::{sync::atomic::Ordering, time::Duration};

use orchester_core::{
  connection::{AgentProvider, DecisionConnection, DecisionServerUrl, stream::AgentEvent},
  id::{DecisionConnectionId, IdempotencyKey},
  name::Name,
  permission::{AgentPermissions, LaunchPermissions, OutsideCommands},
  resource::ModelId,
  run::{
    PendingToolApproval,
    review::{DecisionTestStatus, ReviewEnd, ReviewRecord},
  },
  store::{ActivityStore, TaskStore},
};
use orchester_runner::decision_test::{DecisionTestError, test_in};
use orchester_store::ProjectDb;
use seeded::at;

fn model() -> ModelId {
  ModelId::new("jev-latest").expect("model")
}

fn connection(db: &mut ProjectDb, url: &str) -> DecisionConnection {
  let connection = DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new("Decision").expect("name"),
    DecisionServerUrl::new(url).expect("url"),
    None,
  );
  db.create_decision_connection(&connection).expect("connection");
  connection
}

fn history(count: usize, permissions: bool) -> (tempfile::TempDir, ProjectDb) {
  let (repo, mut db, run, execution) = seeded::started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  if permissions {
    db.save_invocation_permissions(
      invocation.id,
      &LaunchPermissions::new(AgentPermissions::before_permissions(), OutsideCommands::OrchesterAsks),
    )
    .expect("permissions");
  }
  let messages = [(20, "oldest"), (21, "one"), (22, "two"), (23, "three"), (90, "future")]
    .map(|(time, text)| (at(time), AgentEvent::Message { text: text.to_owned() }));
  db.append_activity(&invocation, &messages).expect("messages");
  for index in 0..=count {
    let approval = PendingToolApproval {
      id: format!("{}:{index}", invocation.id),
      run: run.id(),
      execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: true,
      scope: Some("build the change".to_owned()),
      server: "command".to_owned(),
      action: "cargo build".to_owned(),
      target: Some("E:/historical/worktree".to_owned()),
      requested_at: at(30),
      native: None,
    };
    db.request_tool_approval(&approval).expect("approval");
    // The last request was never reviewed and must never be sent by Test.
    if index == count {
      continue;
    }
    db.record_reviewer_usage(&ReviewRecord {
      run: run.id(),
      execution,
      invocation: invocation.id,
      approval: approval.id,
      provider: AgentProvider::ClaudeCodeCli,
      account: None,
      connection: None,
      model: model(),
      started_at: at(40),
      ended_at: at(50),
      usage: None,
      end: if index.is_multiple_of(2) {
        ReviewEnd::Asked
      } else {
        ReviewEnd::Approved
      },
    })
    .expect("review");
  }
  // Edits made after the run must not become the test's task.
  let task = db.task(run.task()).expect("task").expect("exists");
  let mut details = task.details().clone();
  details.title = Name::new("Edited after the run").expect("title");
  details.brief = "Changed task".to_owned();
  details.acceptance_criteria = vec!["New criteria".to_owned()];
  details.pipeline = Some(run.snapshot().pipeline().resource);
  assert!(matches!(
    db.update_task(task.id(), task.revision(), details).expect("edit task"),
    orchester_core::task::TaskUpdate::Saved(_)
  ));
  (repo, db)
}

#[tokio::test]
async fn decision_test_rebuilds_history_stores_figures_and_limits_concurrency() {
  let server = server::Server::new(Duration::from_millis(100), false).await;
  let (repo, mut db) = history(17, true);
  let connection = connection(&mut db, &server.url);
  let vault = vault::MemoryVault::with(vec![]);
  assert!(server.bodies.lock().is_empty());
  assert!(db.latest_decision_test(connection.id()).expect("no tests").is_none());
  let id = IdempotencyKey::generate();
  let report = test_in(repo.path(), vault.as_ref(), id, (&connection).into(), model(), false)
    .await
    .expect("test");
  assert_eq!(report.status, DecisionTestStatus::Completed);
  assert_eq!((report.requests, report.tested, report.failed), (17, 17, 0));
  assert_eq!(report.input_tokens, 17 * 120);
  assert_eq!(report.previously_asked, Some(9));
  assert_eq!(report.unknown_usage, 0);
  assert!(report.median_ms.is_some_and(|ms| ms >= 90));
  for figures in &report.thresholds {
    let approves = figures.threshold.value() <= 0.9;
    assert_eq!(figures.approves, if approves { 17 } else { 0 });
    assert_eq!(figures.asks_person, if approves { 0 } else { 17 });
    assert_eq!(figures.previously_asked_now_approved, if approves { 9 } else { 0 });
  }
  assert_eq!(db.decision_test(id).expect("stored"), Some(report));
  assert_eq!(server.peak.load(Ordering::SeqCst), 8);
  let replay = test_in(repo.path(), vault.as_ref(), id, (&connection).into(), model(), false).await;
  assert!(matches!(replay, Err(DecisionTestError::AlreadySubmitted)));
  let bodies = server.bodies.lock();
  assert_eq!(bodies.len(), 17);
  let state = &bodies[0]["state"];
  assert_eq!(state["task"]["title"], "Say hello");
  assert_eq!(state["step"]["step"], "Implement");
  assert_eq!(
    state["agent_data"]["data"]["agent_last_messages"],
    serde_json::json!(["one", "two", "three"])
  );
  assert_eq!(state["agent_data"]["data"]["agent_note"], "build the change");
  assert_eq!(state["agent_data"]["data"]["working_folder"], "E:/historical/worktree");
  assert!(state["step"].get("network").is_some());
}

#[tokio::test]
async fn decision_test_stop_keeps_progress_and_aborts_inflight_requests() {
  let server = server::Server::new(Duration::from_millis(300), false).await;
  let (repo, mut db) = history(40, true);
  let connection = connection(&mut db, &server.url);
  let vault = vault::MemoryVault::with(vec![]);
  let id = IdempotencyKey::generate();
  let work = test_in(repo.path(), vault.as_ref(), id, (&connection).into(), model(), false);
  let stop = async {
    loop {
      if db
        .decision_test(id)
        .expect("progress")
        .is_some_and(|report| report.tested > 0)
      {
        break;
      }
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(db.stop_decision_test(id).expect("stop"));
  };
  let (report, ()) = tokio::join!(work, stop);
  let report = report.expect("stopped");
  assert_eq!(report.status, DecisionTestStatus::Stopped);
  assert!(report.tested > 0 && report.tested < 40);
  assert_eq!(report.previously_asked, Some(20));
  let sent = server.bodies.lock().len();
  tokio::time::sleep(Duration::from_millis(350)).await;
  assert_eq!(server.bodies.lock().len(), sent);
  assert_eq!(db.decision_test(id).expect("stored"), Some(report));
}

#[tokio::test]
async fn decision_test_remote_automation_is_refused_before_sending() {
  let (repo, mut db) = history(1, true);
  let connection = connection(&mut db, "https://decision.example");
  let vault = vault::MemoryVault::with(vec![]);
  let result = test_in(
    repo.path(),
    vault.as_ref(),
    IdempotencyKey::generate(),
    (&connection).into(),
    model(),
    true,
  )
  .await;
  assert!(matches!(result, Err(DecisionTestError::PersonOnly)));
  assert!(db.latest_decision_test(connection.id()).expect("no report").is_none());
}

#[tokio::test]
async fn decision_test_missing_context_and_failed_answers_are_explicit() {
  let server = server::Server::new(Duration::ZERO, true).await;
  let (repo, mut db) = history(2, false);
  let connection = connection(&mut db, &server.url);
  let vault = vault::MemoryVault::with(vec![]);
  let report = test_in(
    repo.path(),
    vault.as_ref(),
    IdempotencyKey::generate(),
    (&connection).into(),
    model(),
    true,
  )
  .await
  .expect("test");
  assert_eq!((report.tested, report.failed, report.unknown_usage), (2, 2, 2));
  assert!(
    report
      .thresholds
      .iter()
      .all(|figures| figures.approves == 0 && figures.asks_person == 2)
  );
  let bodies = server.bodies.lock();
  let state = &bodies[0]["state"];
  assert!(state["step"].get("repository").is_none());
  assert!(state["step"].get("network").is_none());
  assert!(
    state["not_stored"]
      .as_array()
      .expect("omissions")
      .contains(&serde_json::json!("/step/network"))
  );
}

#[tokio::test]
async fn decision_test_empty_project_finishes_without_sending() {
  let server = server::Server::new(Duration::ZERO, false).await;
  let (repo, mut db) = history(0, true);
  let connection = connection(&mut db, &server.url);
  let vault = vault::MemoryVault::with(vec![]);
  let report = test_in(
    repo.path(),
    vault.as_ref(),
    IdempotencyKey::generate(),
    (&connection).into(),
    model(),
    false,
  )
  .await
  .expect("empty test");
  assert_eq!(report.tested, 0);
  assert_eq!(report.median_ms, None);
  assert_eq!(report.status, DecisionTestStatus::Completed);
  assert!(server.bodies.lock().is_empty());
}

#[tokio::test]
async fn decision_test_overlapping_submissions_are_refused_and_dropped_work_is_interrupted() {
  let server = server::Server::new(Duration::from_millis(300), false).await;
  let (repo, mut db) = history(12, true);
  let connection = connection(&mut db, &server.url);
  let vault = vault::MemoryVault::with(vec![]);
  let id = IdempotencyKey::generate();
  let observe = async {
    loop {
      if !server.bodies.lock().is_empty() {
        break;
      }
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let other = test_in(
      repo.path(),
      vault.as_ref(),
      IdempotencyKey::generate(),
      (&connection).into(),
      model(),
      false,
    )
    .await;
    assert!(matches!(other, Err(DecisionTestError::AlreadySubmitted)));
  };
  tokio::select! {
    result = test_in(repo.path(), vault.as_ref(), id, (&connection).into(), model(), false) => panic!("ended before drop: {result:?}"),
    () = observe => {}
  }
  let report = db.decision_test(id).expect("progress").expect("report");
  assert_eq!(report.status, DecisionTestStatus::Interrupted);
  assert!(report.unknown_usage > 0);
  assert_eq!(report.in_flight, 0);
  let mut stale = report;
  stale.id = IdempotencyKey::generate();
  stale.status = DecisionTestStatus::Running;
  assert!(db.begin_decision_test(&stale).expect("released slot"));
  db.interrupt_decision_tests().expect("new runner");
  assert_eq!(
    db.decision_test(stale.id).expect("read").expect("report").status,
    DecisionTestStatus::Interrupted
  );
}

#[tokio::test]
async fn decision_test_legacy_task_is_omitted_instead_of_using_the_current_task() {
  let server = server::Server::new(Duration::ZERO, false).await;
  let (repo, mut db) = history(1, true);
  let connection = connection(&mut db, &server.url);
  rusqlite::Connection::open(repo.path().join(".orchester/orchester.db"))
    .expect("raw")
    .execute_batch("DROP TRIGGER run_task_snapshots_no_delete; DELETE FROM run_task_snapshots;")
    .expect("legacy run without snapshot");
  let vault = vault::MemoryVault::with(vec![]);
  let report = test_in(
    repo.path(),
    vault.as_ref(),
    IdempotencyKey::generate(),
    (&connection).into(),
    model(),
    false,
  )
  .await
  .expect("test");
  assert!(report.omitted_fields.contains(&"/task".to_owned()));
  assert!(server.bodies.lock()[0]["state"].get("task").is_none());
}

#[tokio::test]
async fn decision_test_stale_endpoint_or_key_is_refused_before_sending() {
  let original = server::Server::new(Duration::ZERO, false).await;
  let replacement = server::Server::new(Duration::ZERO, false).await;
  let (repo, mut db) = history(1, true);
  let connection = connection(&mut db, &original.url);
  let vault = vault::MemoryVault::with(vec![]);
  for (url, key) in [
    (replacement.url.as_str(), None),
    (original.url.as_str(), Some(orchester_core::id::SecretId::generate())),
  ] {
    // Submit the displayed snapshot, then edit the stored connection before execution.
    let command = orchester_ipc::Command::TestDecisionConnection {
      test: IdempotencyKey::generate(),
      connection: (&connection).into(),
      model: model(),
      automation: false,
    };
    let serialized = serde_json::to_vec(&command).expect("serialize submission");
    let mut edited = connection.clone();
    edited.set_server(DecisionServerUrl::new(url).expect("URL"), key);
    db.save_decision_connection(&edited).expect("edit connection");
    let orchester_ipc::Command::TestDecisionConnection {
      test,
      connection: target,
      model,
      automation,
    } = serde_json::from_slice(&serialized).expect("receive submission")
    else {
      panic!("expected Test");
    };
    let result = test_in(repo.path(), vault.as_ref(), test, target, model, automation).await;
    assert!(matches!(result, Err(DecisionTestError::ConnectionChanged)));
    assert!(db.decision_test(test).expect("no report").is_none());
    assert!(original.bodies.lock().is_empty());
    assert!(replacement.bodies.lock().is_empty());
  }
}
