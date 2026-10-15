use orchester_api::{ErrorCode, OrganizationHistory, Tags, TagsChanged, TaskResponse};
use serde_json::json;

use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_organization_history_actor_rules_and_dependency_cycles() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let tasks = create_tasks(&server, &owner.token, project, 2).await.tasks;
  let a = &tasks[0];
  let b = &tasks[1];
  let tag = tag(&server, &owner.token, project, "Work").await;
  let a=changed(&patch(&server,&owner.token,project,a,json!({"workflow":"in_progress","priority":"urgent","tags":[tag.id,tag.id],"blocking":[{"task":b.id,"gate":"review"}],"blocked_reason":" Waiting for a decision ","actor":"orchester"})).await);
  assert_eq!(a.content.tags, [tag.id]);
  assert_eq!(a.content.blocked_reason.as_deref(), Some("Waiting for a decision"));
  let cycle = patch(
    &server,
    &owner.token,
    project,
    b,
    json!({"blocking":[{"task":a.id,"gate":"merge"}]}),
  )
  .await;
  error(&cycle, 422, ErrorCode::Organization);
  assert_eq!(cycle.json::<serde_json::Value>()["reason"], "dependency_cycle");
  for (changes, reason) in [
    (json!({"blocked_reason":"  "}), "missing_reason"),
    (json!({"tags":[orchester_core::id::TagId::generate()]}), "tag_not_found"),
  ] {
    let response = patch(&server, &owner.token, project, &a, changes).await;
    error(&response, 422, ErrorCode::Organization);
    assert_eq!(response.json::<serde_json::Value>()["reason"], reason);
  }
  let a = changed(
    &patch(
      &server,
      &owner.token,
      project,
      &a,
      json!({"blocked_reason":null,"priority":"low"}),
    )
    .await,
  );
  let response = server
    .request(
      "GET",
      &format!("{}/history", task_path(project, &a)),
      Some(&owner.token),
      None,
    )
    .await;
  assert_eq!(response.status, 200);
  let history = response.json::<OrganizationHistory>();
  assert_eq!(history.changes.len(), 2);
  assert_eq!(
    history.changes[0].actor,
    orchester_core::task::organization::ChangeActor::Orchester
  );
  assert_eq!(history.changes[0].by.id, owner.user.id);
  assert_eq!(history.changes[0].to_tag_names, ["Work"]);
  assert_eq!(
    history.changes[1].actor,
    orchester_core::task::organization::ChangeActor::Person
  );
  assert_eq!(history.changes[1].from, history.changes[0].to);
  assert!(history.changes[1].to.blocked_reason.is_none());
  assert!(
    server
      .database()
      .execute("UPDATE organization_changes SET at=0", [])
      .is_err()
  );
  assert!(
    server
      .database()
      .execute("DELETE FROM organization_changes", [])
      .is_err()
  );
  server.finish().await;
}

#[tokio::test]
async fn s1b_cancel_uses_reported_run_phase_and_core_gate_rules() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let task = create_tasks(&server, &owner.token, project, 1).await.tasks.remove(0);
  server
    .database()
    .execute(
      "INSERT INTO run_status VALUES (?1,?2,1,'running','Implementation',1,1,1)",
      rusqlite::params![task.id.to_string(), owner.user.id.to_string()],
    )
    .expect("seed future S3 run report");
  for phase in [
    "running",
    "waiting",
    "paused",
    "stopped_at_limit",
    "interrupted",
    "queued",
  ] {
    server
      .database()
      .execute("UPDATE run_status SET phase=?1", [phase])
      .expect("phase");
    let response = patch(&server, &owner.token, project, &task, json!({"workflow":"canceled"})).await;
    error(&response, 422, ErrorCode::Organization);
    assert_eq!(response.json::<serde_json::Value>()["reason"], "stop_run_first");
  }
  server
    .database()
    .execute("UPDATE run_status SET phase='succeeded'", [])
    .expect("ended");
  let task = changed(&patch(&server, &owner.token, project, &task, json!({"workflow":"canceled"})).await);
  assert_eq!(
    task.content.workflow,
    orchester_core::task::organization::WorkflowStatus::Canceled
  );
  server
    .database()
    .execute("UPDATE run_status SET phase='running'", [])
    .expect("restart");
  assert_eq!(
    patch(&server, &owner.token, project, &task, json!({"priority":"high"}))
      .await
      .status,
    200
  );
  assert_eq!(
    server
      .request(
        "PUT",
        &format!("{}/run", task_path(project, &task)),
        Some(&owner.token),
        Some(json!({}))
      )
      .await
      .status,
    404
  );
  server.finish().await;
}

#[tokio::test]
async fn s1b_tags_rename_merge_delete_rewrite_tasks_and_freeze_history_names() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let first = tag(&server, &owner.token, project, "Alpha").await;
  let target = tag(&server, &owner.token, project, "beta").await;
  error(
    &server
      .request(
        "POST",
        &format!("{}/tags", path(project)),
        Some(&owner.token),
        Some(json!({"name":"ALPHA"})),
      )
      .await,
    409,
    ErrorCode::TagExists,
  );
  let mut tasks = create_tasks(&server, &owner.token, project, 2).await.tasks;
  for task in &mut tasks {
    *task = changed(
      &patch(
        &server,
        &owner.token,
        project,
        task,
        json!({"tags":[first.id,target.id]}),
      )
      .await,
    );
  }
  let endpoint = format!("{}/tags/{}", path(project), first.id);
  assert_eq!(
    server
      .request("PATCH", &endpoint, Some(&owner.token), Some(json!({"name":"Renamed"})))
      .await
      .status,
    200
  );
  error(
    &server
      .request(
        "POST",
        &format!("{endpoint}/merge"),
        Some(&owner.token),
        Some(json!({"into":first.id})),
      )
      .await,
    409,
    ErrorCode::SameTag,
  );
  let merged = server
    .request(
      "POST",
      &format!("{endpoint}/merge"),
      Some(&owner.token),
      Some(json!({"into":target.id})),
    )
    .await;
  assert_eq!(merged.status, 200);
  let merged = merged.json::<TagsChanged>();
  assert_eq!(merged.tasks.len(), 2);
  assert_eq!(merged.tags.len(), 1);
  for (before, after) in tasks.iter().zip(&merged.tasks) {
    assert_eq!(after.revision.get(), before.revision.get() + 1);
    assert_eq!(after.content.tags, [target.id]);
  }
  let history = server
    .request(
      "GET",
      &format!("{}/history", task_path(project, &tasks[0])),
      Some(&owner.token),
      None,
    )
    .await
    .json::<OrganizationHistory>();
  assert_eq!(history.changes[0].to_tag_names.len(), 2);
  assert!(history.changes[0].to_tag_names.contains(&"Alpha".into()));
  assert!(history.changes[1].from_tag_names.contains(&"Renamed".into()));
  assert_tag_removed(&server, &owner.token, project, target.id, &tasks[0]).await;
  server.finish().await;
}

async fn assert_tag_removed(
  server: &TestServer,
  token: &orchester_api::Credential,
  project: orchester_core::id::ProjectId,
  tag: orchester_core::id::TagId,
  task: &orchester_api::SharedTask,
) {
  let removed = server
    .request("DELETE", &format!("{}/tags/{}", path(project), tag), Some(token), None)
    .await;
  assert_eq!(removed.status, 200);
  let removed = removed.json::<TagsChanged>();
  assert!(removed.tags.is_empty());
  assert_eq!(removed.tasks.len(), 2);
  for task in &removed.tasks {
    assert!(task.content.tags.is_empty());
  }
  let kinds: Vec<String> = server
    .database()
    .prepare("SELECT kind FROM events ORDER BY sequence DESC LIMIT 3")
    .expect("query")
    .query_map([], |row| row.get(0))
    .expect("events")
    .collect::<Result<_, _>>()
    .expect("event kinds");
  assert_eq!(kinds, ["tags", "task", "task"]);
  assert!(
    server
      .request("GET", &format!("{}/tags", path(project)), Some(token), None)
      .await
      .json::<Tags>()
      .tags
      .is_empty()
  );
  assert!(
    server
      .request("GET", &task_path(project, task), Some(token), None)
      .await
      .json::<TaskResponse>()
      .task
      .content
      .tags
      .is_empty()
  );
}

#[tokio::test]
async fn s1b_upload_cycle_rolls_back_project_tags_and_history() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let mut a = upload(3);
  let mut b = upload(4);
  a["tag_names"] = json!(["Created then rolled back"]);
  a["blocking"] = json!([{"task":b["id"],"gate":"review"}]);
  b["blocking"] = json!([{"task":a["id"],"gate":"merge"}]);
  let response = server
    .request(
      "POST",
      "/projects",
      Some(&owner.token),
      Some(json!({"idempotency_key":key(),"name":"bad","slug":"bad","key":"BAD","tasks":[a,b]})),
    )
    .await;
  error(&response, 422, ErrorCode::Organization);
  for table in ["projects", "tasks", "tags", "organization_changes", "events"] {
    assert_eq!(
      server
        .database()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get::<_, i64>(0))
        .expect("rollback count"),
      0
    );
  }
  server.finish().await;
}
