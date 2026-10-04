use orchester_api::{ErrorCode, HeartbeatResponse, TaskResponse};
use serde_json::json;

use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_concurrent_claim_staleness_takeover_and_lost_heartbeat() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = user(&server, "bob", false).await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  add_member(&server, &owner.token, project, "bob", "member").await;
  let task = create_tasks(&server, &owner.token, project, 1).await.tasks.remove(0);
  let endpoint = format!("{}/claim", task_path(project, &task));
  let (a, b) = tokio::join!(
    server.request("POST", &endpoint, Some(&owner.token), Some(json!({"kind":"manual"}))),
    server.request("POST", &endpoint, Some(&bob.token), Some(json!({"kind":"manual"})))
  );
  let (winner, loser, winner_token, loser_token) = if a.status == 200 {
    (a, b, &owner.token, &bob.token)
  } else {
    (b, a, &bob.token, &owner.token)
  };
  assert_eq!(winner.status, 200);
  error(&loser, 409, ErrorCode::ClaimHeld);
  let claim = winner.json::<TaskResponse>().task.claim.expect("claim");
  let takeover = format!("{endpoint}/take-over");
  error(
    &server
      .request("POST", &takeover, Some(loser_token), Some(json!({"claim":claim.id})))
      .await,
    409,
    ErrorCode::ClaimNotStale,
  );
  assert_stale_boundary(&server, loser_token, project, &task).await;
  let refreshed = server
    .request(
      "POST",
      "/heartbeat",
      Some(winner_token),
      Some(json!({"tasks":[task.id]})),
    )
    .await;
  assert_eq!(refreshed.status, 200);
  assert!(refreshed.json::<HeartbeatResponse>().lost.is_empty());
  error(
    &server
      .request("POST", &takeover, Some(loser_token), Some(json!({"claim":claim.id})))
      .await,
    409,
    ErrorCode::ClaimNotStale,
  );
  server.clock.advance(15 * 60 * 1000);
  let next = changed(
    &server
      .request("POST", &takeover, Some(loser_token), Some(json!({"claim":claim.id})))
      .await,
  );
  let next_claim = next.claim.expect("new claim");
  assert_ne!(next_claim.id, claim.id);
  assert_eq!(next_claim.taken_over_from.expect("old holder").id, claim.holder.id);
  error(
    &server
      .request("POST", &takeover, Some(winner_token), Some(json!({"claim":claim.id})))
      .await,
    409,
    ErrorCode::ClaimChanged,
  );
  let lost = server
    .request(
      "POST",
      "/heartbeat",
      Some(winner_token),
      Some(json!({"tasks":[task.id]})),
    )
    .await
    .json::<HeartbeatResponse>();
  assert_eq!(lost.lost.len(), 1);
  assert_eq!(lost.lost[0].claim.as_ref().expect("claim").id, next_claim.id);
  assert_audit(&server, "claim_taken_over", 1);
  assert!(server.database().execute("DELETE FROM claim_events", []).is_err());
  server.finish().await;
}

async fn assert_stale_boundary(
  server: &TestServer,
  token: &orchester_api::Credential,
  project: orchester_core::id::ProjectId,
  task: &orchester_api::SharedTask,
) {
  server.clock.advance(15 * 60 * 1000 - 1);
  let fresh = server
    .request("GET", &task_path(project, task), Some(token), None)
    .await
    .json::<TaskResponse>()
    .task;
  assert!(!fresh.claim.expect("fresh").stale);
  server.clock.advance(1);
  let stale = server
    .request("GET", &task_path(project, task), Some(token), None)
    .await
    .json::<TaskResponse>()
    .task;
  assert!(stale.claim.expect("stale remains held").stale);
}

#[tokio::test]
async fn s1b_claim_upgrade_release_and_merged_task_refusal() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let task = create_tasks(&server, &owner.token, project, 1).await.tasks.remove(0);
  let endpoint = format!("{}/claim", task_path(project, &task));
  let manual = changed(
    &server
      .request("POST", &endpoint, Some(&owner.token), Some(json!({"kind":"manual"})))
      .await,
  );
  let retry = changed(
    &server
      .request("POST", &endpoint, Some(&owner.token), Some(json!({"kind":"manual"})))
      .await,
  );
  assert_eq!(retry.revision, manual.revision);
  let run = changed(
    &server
      .request("POST", &endpoint, Some(&owner.token), Some(json!({"kind":"run"})))
      .await,
  );
  assert!(run.has_run);
  assert_eq!(run.claim.as_ref().expect("claim").id, manual.claim.expect("claim").id);
  server
    .database()
    .execute(
      "INSERT INTO run_status VALUES (?1,?2,1,'running',NULL,1,1,1)",
      rusqlite::params![task.id.to_string(), owner.user.id.to_string()],
    )
    .expect("future run");
  error(
    &server.request("DELETE", &endpoint, Some(&owner.token), None).await,
    422,
    ErrorCode::NotEditable,
  );
  server
    .database()
    .execute("UPDATE run_status SET phase='paused'", [])
    .expect("pause");
  let released = changed(&server.request("DELETE", &endpoint, Some(&owner.token), None).await);
  assert!(released.claim.is_none());
  assert!(released.run.is_none());
  error(
    &server.request("DELETE", &endpoint, Some(&owner.token), None).await,
    409,
    ErrorCode::NotHolder,
  );
  server
    .database()
    .execute(
      "INSERT INTO run_status VALUES (?1,?2,1,'merged',NULL,1,1,1)",
      rusqlite::params![task.id.to_string(), owner.user.id.to_string()],
    )
    .expect("seed completed run");
  error(
    &server
      .request("POST", &endpoint, Some(&owner.token), Some(json!({"kind":"run"})))
      .await,
    409,
    ErrorCode::TaskMerged,
  );
  server.finish().await;
}

#[tokio::test]
async fn s1b_member_removal_and_user_disabling_release_claims_atomically() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = user(&server, "bob", false).await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  add_member(&server, &owner.token, project, "bob", "member").await;
  let tasks = create_tasks(&server, &owner.token, project, 2).await.tasks;
  for task in &tasks {
    assert_eq!(
      server
        .request(
          "POST",
          &format!("{}/claim", task_path(project, task)),
          Some(&bob.token),
          Some(json!({"kind":"run"}))
        )
        .await
        .status,
      200
    );
  }
  assert_eq!(
    server
      .request(
        "DELETE",
        &format!("{}/members/{}", path(project), bob.user.id),
        Some(&owner.token),
        None
      )
      .await
      .status,
    204
  );
  assert_unclaimed(&server, &owner.token, project, &tasks).await;
  add_member(&server, &owner.token, project, "bob", "member").await;
  assert_eq!(
    server
      .request(
        "POST",
        &format!("{}/claim", task_path(project, &tasks[0])),
        Some(&bob.token),
        Some(json!({"kind":"manual"}))
      )
      .await
      .status,
    200
  );
  assert_eq!(
    server
      .request(
        "PATCH",
        &format!("/admin/users/{}", bob.user.id),
        Some(&owner.token),
        Some(json!({"disabled":true}))
      )
      .await
      .status,
    200
  );
  assert_unclaimed(&server, &owner.token, project, &tasks).await;
  assert_eq!(
    server
      .database()
      .query_row("SELECT COUNT(*) FROM claim_events WHERE action='released'", [], |row| {
        row.get::<_, i64>(0)
      })
      .expect("release history"),
    3
  );
  server.finish().await;
}

async fn assert_unclaimed(
  server: &TestServer,
  token: &orchester_api::Credential,
  project: orchester_core::id::ProjectId,
  tasks: &[orchester_api::SharedTask],
) {
  for task in tasks {
    let response = server
      .request("GET", &task_path(project, task), Some(token), None)
      .await;
    assert_eq!(response.status, 200);
    assert!(response.json::<TaskResponse>().task.claim.is_none());
  }
}
