use orchester_api::{ErrorCode, TaskResponse};
use serde_json::json;

use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_claim_transitions_preserve_revision_and_holder_take_refreshes_staleness() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  add_member(&server, &owner.token, project, "bob", "member").await;
  let task = create_tasks(&server, &owner.token, project, 1).await.tasks.remove(0);
  let endpoint = format!("{}/claim", task_path(project, &task));
  let manual = changed(
    &server
      .request("POST", &endpoint, Some(&owner.token), Some(json!({"kind":"manual"})))
      .await,
  );
  assert_eq!(manual.revision, task.revision);
  let claim = manual.claim.expect("claim");
  server.clock.advance(16 * 60 * 1000);
  let run = changed(
    &server
      .request("POST", &endpoint, Some(&owner.token), Some(json!({"kind":"run"})))
      .await,
  );
  let refreshed = run.claim.as_ref().expect("claim");
  assert!(!refreshed.stale);
  assert_eq!(refreshed.id, claim.id);
  assert!(refreshed.heartbeat_at > claim.heartbeat_at);
  assert_eq!(run.revision, task.revision);
  let edited = changed(
    &patch(
      &server,
      &bob.token,
      project,
      &task,
      json!({"title":"Concurrent editor"}),
    )
    .await,
  );
  assert_eq!(edited.revision.get(), task.revision.get() + 1);
  error(
    &server
      .request(
        "POST",
        &format!("{endpoint}/take-over"),
        Some(&bob.token),
        Some(json!({"claim":claim.id})),
      )
      .await,
    409,
    ErrorCode::ClaimNotStale,
  );
  server.clock.advance(15 * 60 * 1000);
  let takeover = changed(
    &server
      .request(
        "POST",
        &format!("{endpoint}/take-over"),
        Some(&bob.token),
        Some(json!({"claim":claim.id})),
      )
      .await,
  );
  assert_eq!(takeover.revision, edited.revision);
  let released = changed(&server.request("DELETE", &endpoint, Some(&bob.token), None).await);
  assert_eq!(released.revision, edited.revision);
  let final_task = server
    .request("GET", &task_path(project, &task), Some(&owner.token), None)
    .await
    .json::<TaskResponse>()
    .task;
  assert_eq!(final_task.revision, edited.revision);
  assert!(final_task.claim.is_none());
  server.finish().await;
}
