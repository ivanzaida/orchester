use orchester_server::clock::Clock;
use serde_json::json;

use super::events::Stream;
use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_replay_batches_preserve_all_3000_ordered_events() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![]).await;
  server.database().execute("WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<3000) INSERT INTO events(project,kind,payload,at) SELECT ?1,'project',json_object('id',?1,'key','ORC','name','Replay '||i),?2 FROM n",rusqlite::params![created.project.id.to_string(),server.clock.now_ms()]).expect("persisted replay fixture");
  let mut stream = Stream::open(&server, &owner.token, created.project.id, created.cursor).await;
  for i in 1..=3000 {
    let frame = stream.next().await;
    assert_eq!(frame.id, Some(created.cursor + i));
    assert_eq!(frame.kind.as_deref(), Some("project"));
    assert_eq!(
      serde_json::from_str::<serde_json::Value>(&frame.data).expect("snapshot")["name"],
      format!("Replay {i}")
    );
  }
  drop(stream);
  server.finish().await;
}

#[tokio::test]
async fn s1b_replay_large_snapshots_cross_byte_limited_batches_without_loss() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![]).await;
  for title in ["Large first", "Large second"] {
    let mut task = content(title);
    task["brief"] = json!("b".repeat(900_000));
    assert_eq!(
      server
        .request(
          "POST",
          &format!("{}/tasks", path(created.project.id)),
          Some(&owner.token),
          Some(json!({"idempotency_key":key(),"tasks":[task]}))
        )
        .await
        .status,
      201
    );
  }
  let mut stream = Stream::open(&server, &owner.token, created.project.id, created.cursor).await;
  for title in ["Large first", "Large second"] {
    let frame = stream.next().await;
    assert_eq!(frame.kind.as_deref(), Some("task"));
    let task: orchester_api::SharedTask = serde_json::from_str(&frame.data).expect("large snapshot");
    assert_eq!(task.content.title, title);
    assert_eq!(task.content.brief.len(), 900_000);
  }
  drop(stream);
  server.finish().await;
}
