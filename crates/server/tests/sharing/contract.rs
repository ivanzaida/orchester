use orchester_api::{ErrorCode, TaskResponse};
use serde_json::json;

use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_unknown_request_fields_are_rejected_for_every_shared_request_shape() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![]).await;
  let project = created.project.id;
  let task = create_tasks(&server, &owner.token, project, 1).await.tasks.remove(0);
  let tag = tag(&server, &owner.token, project, "Tag").await;
  let base = path(project);
  let task = task_path(project, &task);
  let tag = format!("{base}/tags/{}", tag.id);
  for (method, path, body) in unknown_requests(&base, &task, &tag, owner.user.id) {
    error(
      &server.request(method, &path, Some(&owner.token), Some(body)).await,
      400,
      ErrorCode::BadRequest,
    );
  }
  error(
    &server
      .request("GET", "/projects?extra=1", Some(&owner.token), None)
      .await,
    400,
    ErrorCode::BadRequest,
  );
  assert_eq!(
    server
      .database()
      .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get::<_, i64>(0))
      .expect("no unexpected projects"),
    1
  );
  server.finish().await;
}

#[tokio::test]
async fn s1b_number_and_limits_preserve_core_ranges() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![upload(u32::MAX)]).await;
  let project = created.project.id;
  assert_eq!(created.project.next_number, u64::from(u32::MAX) + 1);
  assert_eq!(created.tasks[0].number, u32::MAX);
  error(
    &server
      .request(
        "POST",
        &format!("{}/tasks", path(project)),
        Some(&owner.token),
        Some(json!({"idempotency_key":key(),"tasks":[content("No number left")]})),
      )
      .await,
    422,
    ErrorCode::Invalid,
  );
  // A time limit is kept as whole milliseconds in a signed 64-bit integer (core's `LONGEST_TIME_LIMIT`).
  let longest = i64::MAX.unsigned_abs();
  error(
    &patch(
      &server,
      &owner.token,
      project,
      &created.tasks[0],
      json!({"limits":{"max_duration_ms":longest + 1}}),
    )
    .await,
    422,
    ErrorCode::Invalid,
  );
  let changed = changed(
    &patch(
      &server,
      &owner.token,
      project,
      &created.tasks[0],
      json!({"limits":{"budget_tokens":u64::MAX,"max_duration_ms":longest}}),
    )
    .await,
  );
  assert_eq!(
    changed.content.limits.as_ref().expect("limits").budget_tokens,
    Some(u64::MAX)
  );
  let read = server
    .request("GET", &task_path(project, &changed), Some(&owner.token), None)
    .await
    .json::<TaskResponse>()
    .task;
  assert_eq!(read.content.limits.expect("limits").max_duration_ms, Some(longest));
  server.finish().await;
}

fn unknown_requests(
  base: &str,
  task: &str,
  tag: &str,
  owner: orchester_core::id::UserId,
) -> Vec<(&'static str, String, serde_json::Value)> {
  let mut upload = upload(9);
  upload["extra"] = json!(1);
  vec![
    (
      "POST",
      "/projects".into(),
      json!({"idempotency_key":key(),"name":"extra","slug":"extra","key":"EX","tasks":[],"extra":1}),
    ),
    (
      "POST",
      "/projects".into(),
      json!({"idempotency_key":key(),"name":"extra","slug":"extra","key":"EX","tasks":[upload]}),
    ),
    ("PATCH", base.to_owned(), json!({"name":"extra","extra":1})),
    (
      "POST",
      format!("{base}/members"),
      json!({"username":"alice","role":"owner","extra":1}),
    ),
    (
      "PATCH",
      format!("{base}/members/{}", owner),
      json!({"role":"member","extra":1}),
    ),
    (
      "POST",
      format!("{base}/tasks"),
      json!({"idempotency_key":key(),"tasks":[],"extra":1}),
    ),
    (
      "POST",
      format!("{base}/tasks/import"),
      json!({"idempotency_key":key(),"tasks":[],"extra":1}),
    ),
    ("PATCH", task.to_owned(), json!({"revision":1,"changes":{},"extra":1})),
    (
      "PATCH",
      task.to_owned(),
      json!({"revision":1,"changes":{"limits":{"budget_tokens":1,"max_duration_ms":1,"extra":1}}}),
    ),
    (
      "PATCH",
      task.to_owned(),
      json!({"revision":1,"changes":{"blocking":[{"task":orchester_core::id::TaskId::generate(),"gate":"review","extra":1}]}}),
    ),
    ("POST", format!("{task}/claim"), json!({"kind":"manual","extra":1})),
    (
      "POST",
      format!("{task}/claim/take-over"),
      json!({"claim":orchester_core::id::ClaimId::generate(),"extra":1}),
    ),
    ("POST", "/heartbeat".into(), json!({"tasks":[],"extra":1})),
    ("POST", format!("{base}/tags"), json!({"name":"extra","extra":1})),
    ("PATCH", tag.to_owned(), json!({"name":"extra","extra":1})),
    (
      "POST",
      format!("{tag}/merge"),
      json!({"into":orchester_core::id::TagId::generate(),"extra":1}),
    ),
  ]
}
