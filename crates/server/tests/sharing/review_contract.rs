use orchester_api::{ErrorCode, OrganizationHistory, ProjectResponse, SharingError, SharingErrorDetails, Tasks};
use serde_json::json;

use super::events::Stream;
use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_batch_references_fail_before_foreign_keys_and_roll_back_numbers() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let endpoint = format!("{}/tasks", path(project));
  let unknown_tag = orchester_core::id::TagId::generate();
  let unknown_task = orchester_core::id::TaskId::generate();
  for (field, value, code) in [
    ("tags", json!([unknown_tag]), ErrorCode::Organization),
    (
      "blocking",
      json!([{"task":unknown_task,"gate":"review"}]),
      ErrorCode::Invalid,
    ),
  ] {
    let mut task = content("Unknown reference");
    task[field] = value;
    let response = server
      .request(
        "POST",
        &endpoint,
        Some(&owner.token),
        Some(json!({"idempotency_key":key(),"tasks":[content("First"),task]})),
      )
      .await;
    error(&response, 422, code);
    if code == ErrorCode::Organization {
      assert!(
        matches!(response.json::<SharingError>().details,SharingErrorDetails::Reason {reason} if reason=="tag_not_found")
      );
    }
  }
  assert!(
    server
      .request("GET", &endpoint, Some(&owner.token), None)
      .await
      .json::<Tasks>()
      .tasks
      .is_empty()
  );
  assert_eq!(create_tasks(&server, &owner.token, project, 1).await.tasks[0].number, 1);
  server.finish().await;
}

#[tokio::test]
async fn s1b_finished_run_upload_and_import_remain_editable() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let mut input = upload(5);
  input["status"] = json!("queued");
  input["has_run"] = json!(true);
  let created = project(&server, &owner.token, vec![input]).await;
  let task = &created.tasks[0];
  assert!(task.has_run && task.claim.is_none());
  let edited = changed(
    &patch(
      &server,
      &owner.token,
      created.project.id,
      task,
      json!({"title":"Edited finished task"}),
    )
    .await,
  );
  assert_eq!(edited.content.title, "Edited finished task");
  let mut imported = upload(6);
  imported["status"] = json!("queued");
  imported["has_run"] = json!(true);
  let tasks = server
    .request(
      "POST",
      &format!("{}/tasks/import", path(created.project.id)),
      Some(&owner.token),
      Some(json!({"idempotency_key":key(),"tasks":[imported]})),
    )
    .await
    .json::<orchester_api::TasksCreated>();
  let edited = changed(
    &patch(
      &server,
      &owner.token,
      created.project.id,
      &tasks.tasks[0],
      json!({"brief":"New brief"}),
    )
    .await,
  );
  assert!(edited.has_run && edited.claim.is_none());
  server.finish().await;
}

#[tokio::test]
async fn s1b_duplicate_links_have_identical_history_and_task_organization() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let tasks = create_tasks(&server, &owner.token, project, 2).await.tasks;
  let edited = changed(
    &patch(
      &server,
      &owner.token,
      project,
      &tasks[0],
      json!({"blocking":[{"task":tasks[1].id,"gate":"review"},{"task":tasks[1].id,"gate":"merge"}]}),
    )
    .await,
  );
  assert_eq!(edited.content.blocking.len(), 1);
  assert_eq!(
    edited.content.blocking[0].gate,
    orchester_core::task::organization::DependencyGate::Merge
  );
  let history = server
    .request(
      "GET",
      &format!("{}/history", task_path(project, &edited)),
      Some(&owner.token),
      None,
    )
    .await
    .json::<OrganizationHistory>();
  let links = &history.changes.last().expect("change").to.blocking;
  assert_eq!(links, &edited.content.blocking);
  server.finish().await;
}

#[tokio::test]
async fn s1b_members_project_events_and_admin_relationship_match_contract() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = user(&server, "bob", false).await;
  let admin = user(&server, "admin", true).await;
  let created = project(&server, &owner.token, vec![]).await;
  let project = created.project.id;
  let lookup = server
    .request(
      "GET",
      "/projects?remote=host.example/owner/repo",
      Some(&bob.token),
      None,
    )
    .await
    .json::<SharingError>();
  assert!(
    matches!(lookup.details,SharingErrorDetails::Owners {owners} if owners==vec![orchester_api::UserRef {id:owner.user.id,display_name:owner.user.display_name.clone()}])
  );
  let mut stream = Stream::open(&server, &owner.token, project, created.cursor).await;
  add_member(&server, &owner.token, project, "bob", "member").await;
  let added = stream.next().await;
  assert_eq!(added.kind.as_deref(), Some("members"));
  assert!(added.data.contains(&bob.user.id.to_string()));
  assert_eq!(
    server
      .request(
        "PATCH",
        &format!("{}/members/{}", path(project), bob.user.id),
        Some(&admin.token),
        Some(json!({"role":"owner"}))
      )
      .await
      .status,
    200
  );
  let roles = stream.next().await;
  assert_eq!(roles.kind.as_deref(), Some("members"));
  let members: orchester_api::Members = serde_json::from_str(&roles.data).expect("members event");
  assert_eq!(
    members
      .members
      .iter()
      .find(|m| m.user.id == bob.user.id)
      .expect("bob")
      .role,
    orchester_api::MemberRole::Owner
  );
  let patched = server
    .request(
      "PATCH",
      &path(project),
      Some(&admin.token),
      Some(json!({"name":"Renamed","key":"NEW"})),
    )
    .await
    .json::<ProjectResponse>();
  assert_eq!(patched.project.role, None);
  let event = stream.next().await;
  assert_eq!(event.kind.as_deref(), Some("project"));
  let value: serde_json::Value = serde_json::from_str(&event.data).expect("project event");
  assert_eq!(value, json!({"id":project,"key":"NEW","name":"Renamed"}));
  drop(stream);
  server.finish().await;
}

#[tokio::test]
async fn s1b_import_cross_project_collision_returns_generic_conflict() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![upload(1)]).await;
  let bob = user(&server, "bob", false).await;
  let other = server
    .request(
      "POST",
      "/projects",
      Some(&bob.token),
      Some(json!({"idempotency_key":key(),"key":"BOB","name":"Other","remote":"host.example/other/repo","tasks":[]})),
    )
    .await
    .json::<orchester_api::ProjectCreated>();
  let mut stolen = upload(2);
  stolen["id"] = json!(created.tasks[0].id);
  let response = server
    .request(
      "POST",
      &format!("{}/tasks/import", path(other.project.id)),
      Some(&bob.token),
      Some(json!({"idempotency_key":key(),"tasks":[stolen]})),
    )
    .await;
  error(&response, 409, ErrorCode::Conflict);
  assert!(!response.contains("id_taken") && !response.contains(&created.project.id.to_string()));
  assert_eq!(
    create_tasks(&server, &bob.token, other.project.id, 1).await.tasks[0].number,
    1
  );
  server.finish().await;
}
