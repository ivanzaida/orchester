use orchester_api::{ErrorCode, TaskResponse, TasksCreated};
use serde_json::json;

use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_simultaneous_batch_retries_and_revision_edits_are_atomic() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let endpoint = format!("{}/tasks", path(project));
  let input = json!({"idempotency_key":key(),"tasks":[content("Once")]});
  let (a, b) = tokio::join!(
    server.request("POST", &endpoint, Some(&owner.token), Some(input.clone())),
    server.request("POST", &endpoint, Some(&owner.token), Some(input))
  );
  assert_eq!(a.status, 201);
  assert_eq!(b.status, 201);
  assert_eq!(a.json::<serde_json::Value>(), b.json::<serde_json::Value>());
  let task = a.json::<TasksCreated>().tasks.remove(0);
  let (a, b) = tokio::join!(
    patch(&server, &owner.token, project, &task, json!({"title":"A"})),
    patch(&server, &owner.token, project, &task, json!({"title":"B"}))
  );
  let (winner, loser) = if a.status == 200 { (a, b) } else { (b, a) };
  assert_eq!(winner.status, 200);
  error(&loser, 409, ErrorCode::RevisionConflict);
  let current = server
    .request("GET", &task_path(project, &task), Some(&owner.token), None)
    .await
    .json::<TaskResponse>()
    .task;
  assert_eq!(current.revision.get(), 2);
  assert_eq!(current.content.title, winner.json::<TaskResponse>().task.content.title);
  assert_eq!(create_tasks(&server, &owner.token, project, 1).await.tasks[0].number, 2);
  server.finish().await;
}

#[tokio::test]
async fn s1b_simultaneous_owner_demotions_leave_one_owner() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  add_member(&server, &owner.token, project, "bob", "owner").await;
  let own = format!("{}/members/{}", path(project), owner.user.id);
  let other = format!("{}/members/{}", path(project), bob.user.id);
  let (a, b) = tokio::join!(
    server.request("PATCH", &own, Some(&owner.token), Some(json!({"role":"member"}))),
    server.request("PATCH", &other, Some(&owner.token), Some(json!({"role":"member"})))
  );
  let (winner, loser) = if a.status == 200 { (a, b) } else { (b, a) };
  assert_eq!(winner.status, 200);
  error(&loser, 409, ErrorCode::LastOwner);
  let owners: i64 = server
    .database()
    .query_row("SELECT COUNT(*) FROM memberships WHERE role='owner'", [], |row| {
      row.get(0)
    })
    .expect("remaining owner");
  assert_eq!(owners, 1);
  assert_audit(&server, "member_role_changed", 1);
  server.finish().await;
}

#[tokio::test]
async fn s1b_waiting_writer_uses_transaction_time_for_takeover() {
  use orchester_server::clock::Clock;
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  add_member(&server, &owner.token, project, "bob", "member").await;
  let task = create_tasks(&server, &owner.token, project, 1).await.tasks.remove(0);
  let endpoint = format!("{}/claim", task_path(project, &task));
  let held = changed(
    &server
      .request("POST", &endpoint, Some(&owner.token), Some(json!({"kind":"manual"})))
      .await,
  );
  let claim = held.claim.expect("holder");
  let lock = server.database();
  lock.execute_batch("BEGIN IMMEDIATE").expect("hold writer");
  let takeover = format!("{endpoint}/take-over");
  let (response, ()) = tokio::join!(
    server.request("POST", &takeover, Some(&bob.token), Some(json!({"claim":claim.id}))),
    async {
      tokio::time::sleep(std::time::Duration::from_millis(200)).await;
      server.clock.advance(orchester_core::sharing::CLAIM_STALE_MS);
      lock.execute_batch("COMMIT").expect("release writer");
    }
  );
  let next = changed(&response).claim.expect("takeover");
  assert_eq!(next.holder.id, bob.user.id);
  assert_eq!(next.since, server.clock.now_ms());
  drop(lock);
  server.finish().await;
}

#[tokio::test]
async fn s1b_waiting_writer_rechecks_session_expiry_at_transaction_time() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let task = create_tasks(&server, &owner.token, project, 1).await.tasks.remove(0);
  let lock = server.database();
  lock.execute_batch("BEGIN IMMEDIATE").expect("hold writer");
  let (response, ()) = tokio::join!(
    patch(
      &server,
      &owner.token,
      project,
      &task,
      json!({"title":"Must not commit after expiry"})
    ),
    async {
      tokio::time::sleep(std::time::Duration::from_millis(200)).await;
      server.clock.advance(orchester_server::clock::DEVICE_IDLE_MS);
      lock.execute_batch("COMMIT").expect("release writer");
    }
  );
  error(&response, 401, ErrorCode::Unauthorized);
  assert_eq!(
    server
      .database()
      .query_row("SELECT revision FROM tasks WHERE id=?1", [task.id.to_string()], |row| {
        row.get::<_, i64>(0)
      })
      .expect("unchanged revision"),
    1
  );
  drop(lock);
  server.finish().await;
}
