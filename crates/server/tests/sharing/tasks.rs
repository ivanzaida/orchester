use orchester_api::{ErrorCode, SharingError, SharingErrorDetails, TaskResponse, Tasks, TasksCreated};
use serde_json::json;

use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_five_clients_allocate_250_numbers_without_gaps() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let clients = [
    server
      .login("alice", &crate::support::credential("a secure admin password"))
      .await,
    server
      .login("alice", &crate::support::credential("a secure admin password"))
      .await,
    server
      .login("alice", &crate::support::credential("a secure admin password"))
      .await,
    server
      .login("alice", &crate::support::credential("a secure admin password"))
      .await,
    server
      .login("alice", &crate::support::credential("a secure admin password"))
      .await,
  ];
  let (a, b, c, d, e) = tokio::join!(
    create_tasks(&server, &clients[0].token, project, 50),
    create_tasks(&server, &clients[1].token, project, 50),
    create_tasks(&server, &clients[2].token, project, 50),
    create_tasks(&server, &clients[3].token, project, 50),
    create_tasks(&server, &clients[4].token, project, 50)
  );
  let mut numbers: Vec<_> = [a, b, c, d, e]
    .into_iter()
    .flat_map(|batch| batch.tasks.into_iter().map(|task| task.number))
    .collect();
  numbers.sort();
  assert_eq!(numbers, (1..=250).collect::<Vec<_>>());
  let listed = server
    .request("GET", &format!("{}/tasks", path(project)), Some(&owner.token), None)
    .await
    .json::<Tasks>();
  assert_eq!(listed.tasks.len(), 250);
  assert!(listed.cursor >= 250);
  server.finish().await;
}

#[tokio::test]
async fn s1b_batch_retry_returns_original_snapshot_and_stale_patch_current_task() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let input = json!({"idempotency_key":key(),"tasks":[content("First")]});
  let endpoint = format!("{}/tasks", path(project));
  let first = server
    .request("POST", &endpoint, Some(&owner.token), Some(input.clone()))
    .await;
  assert_eq!(first.status, 201);
  let task = first.json::<TasksCreated>().tasks.remove(0);
  let updated = changed(&patch(&server, &owner.token, project, &task, json!({"title":"Edited"})).await);
  assert_eq!(updated.revision.get(), 2);
  let retry = server.request("POST", &endpoint, Some(&owner.token), Some(input)).await;
  assert_eq!(retry.json::<serde_json::Value>(), first.json::<serde_json::Value>());
  let conflict = patch(&server, &owner.token, project, &task, json!({"brief":"stale"})).await;
  error(&conflict, 409, ErrorCode::RevisionConflict);
  match conflict.json::<SharingError>().details {
    SharingErrorDetails::Revision { current } => {
      assert_eq!(current.content.title, "Edited");
      assert_eq!(current.revision, updated.revision);
    }
    _ => panic!("current task details"),
  }
  server.finish().await;
}

#[tokio::test]
async fn s1b_queue_and_edit_rules_use_core_and_null_patch_is_present() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let created = server
    .request(
      "POST",
      &format!("{}/tasks", path(project)),
      Some(&owner.token),
      Some(json!({"idempotency_key":key(),"tasks":[{"title":"Incomplete"}]})),
    )
    .await;
  let task = created.json::<TasksCreated>().tasks.remove(0);
  let refused = patch(&server, &owner.token, project, &task, json!({"status":"queued"})).await;
  error(&refused, 422, ErrorCode::NotQueueable);
  let task=changed(&patch(&server,&owner.token,project,&task,json!({"brief":"Ready","criteria":["works"],"pipeline":"feature","limits":{"budget_tokens":100,"max_duration_ms":1000},"status":"queued"})).await);
  error(
    &patch(&server, &owner.token, project, &task, json!({"title":"Locked"})).await,
    422,
    ErrorCode::NotEditable,
  );
  let task = changed(&patch(&server, &owner.token, project, &task, json!({"priority":"high"})).await);
  let queued = patch(&server, &owner.token, project, &task, json!({"status":"queued"})).await;
  error(&queued, 422, ErrorCode::NotQueueable);
  assert_eq!(queued.json::<serde_json::Value>()["missing"], json!([]));
  assert_eq!(queued.json::<serde_json::Value>()["reason"], "not_draft");
  let task = changed(
    &server
      .request(
        "POST",
        &format!("{}/claim", task_path(project, &task)),
        Some(&owner.token),
        Some(json!({"kind":"run"})),
      )
      .await,
  );
  assert!(task.has_run);
  let incomplete = patch(&server, &owner.token, project, &task, json!({"pipeline":null})).await;
  error(&incomplete, 422, ErrorCode::NotEditable);
  assert_eq!(incomplete.json::<serde_json::Value>()["reason"], "incomplete");
  let edited = changed(&patch(&server, &owner.token, project, &task, json!({"title":"Editable now"})).await);
  assert_eq!(edited.content.pipeline.as_deref(), Some("feature"));
  let shown = server
    .request("GET", &task_path(project, &task), Some(&owner.token), None)
    .await
    .json::<TaskResponse>();
  assert_eq!(shown.task.content.title, "Editable now");
  server.finish().await;
}

#[tokio::test]
async fn s1b_import_keeps_ids_former_keys_tags_links_and_claims() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![upload(8)]).await.project.id;
  let mut a = upload(45);
  let b = upload(70);
  let missing = orchester_core::id::TaskId::generate();
  a["former_key"] = json!("KON-45");
  a["tag_names"] = json!(["Work", "work"]);
  a["blocking"] = json!([{"task":b["id"],"gate":"review"},{"task":missing,"gate":"merge"}]);
  a["claim"] = json!(true);
  a["status"] = json!("queued");
  let input = json!({"idempotency_key":key(),"tasks":[a.clone(),b.clone()]});
  let endpoint = format!("{}/tasks/import", path(project));
  let response = server
    .request("POST", &endpoint, Some(&owner.token), Some(input.clone()))
    .await;
  assert_eq!(response.status, 201);
  let imported: TasksCreated = response.json();
  assert_eq!(
    imported.tasks.iter().map(|task| task.number).collect::<Vec<_>>(),
    [9, 10]
  );
  assert_eq!(json!(imported.tasks[0].id), a["id"]);
  assert_eq!(imported.tasks[0].former_key.as_deref(), Some("KON-45"));
  assert!(imported.tasks[0].has_run);
  assert!(imported.tasks[0].claim.is_some());
  assert_eq!(imported.tasks[0].content.tags.len(), 1);
  assert_eq!(imported.tasks[0].content.blocking.len(), 1);
  assert_eq!(imported.dropped_links.len(), 1);
  assert_eq!(imported.dropped_links[0].blocking, missing);
  let retry = server.request("POST", &endpoint, Some(&owner.token), Some(input)).await;
  assert_eq!(retry.json::<serde_json::Value>(), response.json::<serde_json::Value>());
  let duplicate = server
    .request(
      "POST",
      &endpoint,
      Some(&owner.token),
      Some(json!({"idempotency_key":key(),"tasks":[upload(1),a]})),
    )
    .await;
  error(&duplicate, 409, ErrorCode::IdTaken);
  assert_eq!(
    create_tasks(&server, &owner.token, project, 1).await.tasks[0].number,
    11
  );
  server.finish().await;
}

#[tokio::test]
async fn s1b_invalid_batches_and_unknown_fields_do_not_write() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let endpoint = format!("{}/tasks", path(project));
  for bad in [
    json!({"title":""}),
    json!({"title":"bad","limits":{"budget_tokens":0,"max_duration_ms":1}}),
    json!({"title":"bad","pipeline":"INVALID"}),
  ] {
    error(
      &server
        .request(
          "POST",
          &endpoint,
          Some(&owner.token),
          Some(json!({"idempotency_key":key(),"tasks":[content("rollback"),bad]})),
        )
        .await,
      422,
      ErrorCode::Invalid,
    );
  }
  error(
    &server
      .request(
        "POST",
        &endpoint,
        Some(&owner.token),
        Some(json!({"idempotency_key":key(),"tasks":[{"title":"bad","unknown":1}]})),
      )
      .await,
    400,
    ErrorCode::BadRequest,
  );
  let task = create_tasks(&server, &owner.token, project, 1).await.tasks.remove(0);
  assert_eq!(task.number, 1);
  error(
    &patch(&server, &owner.token, project, &task, json!({"unknown":1})).await,
    400,
    ErrorCode::BadRequest,
  );
  let before: i64 = server
    .database()
    .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
    .expect("events");
  let same = changed(&patch(&server, &owner.token, project, &task, json!({})).await);
  assert_eq!(same.revision, task.revision);
  assert_eq!(
    server
      .database()
      .query_row("SELECT COUNT(*) FROM events", [], |row| row.get::<_, i64>(0))
      .expect("events"),
    before
  );
  server.finish().await;
}
