use std::time::Duration;

use orchester_api::{ErrorCode, TaskResponse};
use orchester_server::clock::Clock;
use serde_json::json;

use super::events::Stream;
use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_heartbeats_bound_requests_and_never_duplicate_large_task_events() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let mut input = content("Large task");
  input["brief"] = json!("b".repeat(900_000));
  let task = server
    .request(
      "POST",
      &format!("{}/tasks", path(project)),
      Some(&owner.token),
      Some(json!({"idempotency_key":key(),"tasks":[input]})),
    )
    .await
    .json::<orchester_api::TasksCreated>()
    .tasks
    .remove(0);
  changed(
    &server
      .request(
        "POST",
        &format!("{}/claim", task_path(project, &task)),
        Some(&owner.token),
        Some(json!({"kind":"run"})),
      )
      .await,
  );
  let before: (i64, i64) = server
    .database()
    .query_row("SELECT COUNT(*),SUM(length(payload)) FROM events", [], |row| {
      Ok((row.get(0)?, row.get(1)?))
    })
    .expect("event size");
  for _ in 0..12 {
    server.clock.advance(1);
    assert_eq!(
      server
        .request(
          "POST",
          "/heartbeat",
          Some(&owner.token),
          Some(json!({"tasks":[task.id]}))
        )
        .await
        .status,
      200
    );
  }
  let limited = server
    .request(
      "POST",
      "/heartbeat",
      Some(&owner.token),
      Some(json!({"tasks":[task.id]})),
    )
    .await;
  error(&limited, 429, ErrorCode::RateLimited);
  let after = server
    .database()
    .query_row("SELECT COUNT(*),SUM(length(payload)) FROM events", [], |row| {
      Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })
    .expect("no heartbeat payloads");
  assert_eq!(before, after);
  assert_eq!(
    server
      .request("GET", &task_path(project, &task), Some(&owner.token), None)
      .await
      .json::<TaskResponse>()
      .task
      .revision,
    task.revision
  );
  server.finish().await;
}

#[tokio::test]
async fn s1b_heartbeat_limit_expires_and_task_list_is_capped() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  assert_eq!(heartbeat_burst(&server, &owner.token, 13).await, 12);
  let second = server
    .login("alice", &crate::support::credential("a secure admin password"))
    .await;
  assert_eq!(heartbeat_burst(&server, &second.token, 1).await, 1);
  let task = orchester_core::id::TaskId::generate();
  error(
    &server
      .request(
        "POST",
        "/heartbeat",
        Some(&second.token),
        Some(json!({"tasks":vec![task;257]})),
      )
      .await,
    422,
    ErrorCode::Invalid,
  );
  server.clock.advance(60_000);
  assert_eq!(heartbeat_burst(&server, &owner.token, 1).await, 1);
  server.finish().await;
}

#[tokio::test]
async fn s1b_concurrent_heartbeat_requests_share_one_session_allowance() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let (a, b, c) = tokio::join!(
    heartbeat_burst(&server, &owner.token, 10),
    heartbeat_burst(&server, &owner.token, 10),
    heartbeat_burst(&server, &owner.token, 10)
  );
  assert_eq!(a + b + c, 12);
  server.finish().await;
}

async fn heartbeat_burst(server: &TestServer, token: &orchester_api::Credential, count: usize) -> usize {
  let mut accepted = 0;
  for _ in 0..count {
    let response = server
      .request("POST", "/heartbeat", Some(token), Some(json!({"tasks":[]})))
      .await;
    if response.status == 200 {
      accepted += 1;
    } else {
      error(&response, 429, ErrorCode::RateLimited);
      assert!(response.headers["retry-after"].parse::<u64>().expect("retry seconds") > 0);
    }
  }
  accepted
}

#[tokio::test]
async fn s1b_stream_limits_per_session_and_user_release_on_disconnect_and_revocation() {
  let server =
    TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.1".parse().expect("trusted proxy")]).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![]).await;
  let second = server
    .login("alice", &crate::support::credential("a secure admin password"))
    .await;
  let third = server
    .login("alice", &crate::support::credential("a secure admin password"))
    .await;
  let bob = user(&server, "bob", false).await;
  add_member(&server, &owner.token, created.project.id, "bob", "member").await;
  let endpoint = format!("{}/events", path(created.project.id));
  let mut first = Vec::new();
  for _ in 0..8 {
    first.push(Stream::open(&server, &owner.token, created.project.id, created.cursor).await);
  }
  let denied = server.request("GET", &endpoint, Some(&owner.token), None).await;
  error(&denied, 429, ErrorCode::RateLimited);
  assert_eq!(denied.headers["retry-after"], "1");
  let mut others = Vec::new();
  for _ in 0..8 {
    others.push(Stream::open(&server, &second.token, created.project.id, created.cursor).await);
  }
  error(
    &server.request("GET", &endpoint, Some(&third.token), None).await,
    429,
    ErrorCode::RateLimited,
  );
  let other_user = Stream::open(&server, &bob.token, created.project.id, created.cursor).await;
  assert_eq!(server.request("GET", "/server", None, None).await.status, 200);
  drop(others.pop());
  let replacement = tokio::time::timeout(Duration::from_secs(5), async {
    loop {
      match Stream::try_open(&server, &third.token, created.project.id, created.cursor).await {
        Ok(stream) => break stream,
        Err(status) => {
          assert_eq!(status, 429);
          tokio::time::sleep(Duration::from_millis(10)).await;
        }
      }
    }
  })
  .await
  .expect("disconnect releases capacity");
  assert_eq!(
    server
      .request(
        "DELETE",
        &format!("/auth/sessions/{}", owner.session.id),
        Some(&second.token),
        None
      )
      .await
      .status,
    204
  );
  for stream in &mut first {
    while stream.next().await.kind.as_deref() != Some("revoked") {}
    stream.closed().await;
  }
  let renewed = Stream::open(&server, &third.token, created.project.id, created.cursor).await;
  drop((first, others, replacement, renewed, other_user));
  server.finish().await;
}

#[tokio::test]
async fn s1b_retention_prunes_once_per_minute_and_indexes_expired_project_events() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  server.clock.advance(30 * 24 * 60 * 60 * 1000 + 1);
  let fresh = server
    .login("alice", &crate::support::credential("a secure admin password"))
    .await;
  let database = server.database();
  database.execute_batch("CREATE TABLE prune_observations (at INTEGER); CREATE TRIGGER observe_prune AFTER UPDATE OF event_floor ON projects BEGIN INSERT INTO prune_observations VALUES (1); END;").expect("observe retention work");
  database.execute("WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<300000) INSERT INTO events(project,kind,payload,at) SELECT ?1,'tags','[]',?2 FROM n",rusqlite::params![project.to_string(),server.clock.now_ms()]).expect("300k recent events");
  create_tasks(&server, &fresh.token, project, 50).await;
  create_tasks(&server, &fresh.token, project, 50).await;
  assert_eq!(
    database
      .query_row("SELECT COUNT(*) FROM prune_observations", [], |row| row
        .get::<_, i64>(0))
      .expect("prune count"),
    1
  );
  assert_eq!(
    database
      .query_row(
        "SELECT COUNT(*) FROM events WHERE at<?1",
        [server.clock.now_ms() - 30 * 24 * 60 * 60 * 1000],
        |row| row.get::<_, i64>(0)
      )
      .expect("expired rows"),
    0
  );
  let plan: String = database.query_row("EXPLAIN QUERY PLAN SELECT MAX(sequence) FROM events INDEXED BY events_project_retention WHERE project=?1 AND at<?2",rusqlite::params![project.to_string(),server.clock.now_ms()-30*24*60*60*1000],|row|row.get(3)).expect("index plan");
  assert!(plan.contains("events_project_retention") && plan.contains("at<?"));
  server.clock.advance(60_000);
  create_tasks(&server, &fresh.token, project, 1).await;
  assert_eq!(
    database
      .query_row("SELECT COUNT(*) FROM prune_observations", [], |row| row
        .get::<_, i64>(0))
      .expect("next prune"),
    2
  );
  drop(database);
  server.finish().await;
}
