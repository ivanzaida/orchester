use orchester_api::{
  ErrorCode, MemberResponse, ProjectCreated, ProjectLinked, ProjectResponse, SharingError, SharingErrorDetails,
};
use serde_json::json;

use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1c_admins_create_projects_by_slug_with_numbers_kept_and_the_first_result_repeated() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let input = json!({"idempotency_key":key(),"name":" Shared ","slug":"kontur","key":"ORC","remote_hint":"https://user:private-remote-token@HOST.EXAMPLE:443/Owner/Repo.git/","tasks":[upload(7),upload(19)]});
  let response = server
    .request("POST", "/projects", Some(&owner.token), Some(input.clone()))
    .await;
  assert_eq!(response.status, 201);
  assert!(!response.contains("private-remote-token"));
  let created: ProjectCreated = response.json();
  assert_eq!(created.project.slug, "kontur");
  assert_eq!(created.project.address, "http://localhost/kontur");
  assert_eq!(created.project.next_number, 20);
  assert_eq!(created.tasks.iter().map(|t| t.number).collect::<Vec<_>>(), [7, 19]);
  assert_eq!(
    created.project.remote_hint.as_ref().expect("hint").as_str(),
    "host.example/owner/repo"
  );
  assert_eq!(created.tasks[0].created.at, 123);
  let changed = server
    .request(
      "PATCH",
      &path(created.project.id),
      Some(&owner.token),
      Some(json!({"key":"NEW","name":"Renamed","remote_hint":"git@host.example:owner/other.git"})),
    )
    .await;
  let changed: ProjectResponse = changed.json();
  assert_eq!(changed.project.slug, "kontur", "a slug never changes");
  assert_eq!(
    changed.project.remote_hint.expect("hint").as_str(),
    "host.example/owner/other"
  );
  let replay = server
    .request("POST", "/projects", Some(&owner.token), Some(input))
    .await;
  assert_eq!(replay.status, 201);
  assert_eq!(replay.json::<serde_json::Value>(), response.json::<serde_json::Value>());
  let found = server
    .request("GET", "/projects?slug=kontur", Some(&owner.token), None)
    .await;
  assert_eq!(found.json::<ProjectResponse>().project.id, created.project.id);
  let created_task = create_tasks(&server, &owner.token, created.project.id, 1).await;
  assert_eq!(created_task.tasks[0].number, 20);
  assert_eq!(created_task.tasks[0].key, "NEW-20");
  let stored: String = server
    .database()
    .query_row("SELECT remote_hint FROM projects", [], |row| row.get(0))
    .expect("stored remote");
  assert!(!stored.contains("private-remote-token"));
  server.finish().await;
}

#[tokio::test]
async fn s1c_slugs_are_unique_valid_and_not_reserved_and_only_admins_create_projects() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let body = |slug: &str| json!({"idempotency_key":key(),"name":"Kontur","slug":slug,"key":"KON","tasks":[]});
  let create = |token, slug: &str| server.request("POST", "/projects", Some(token), Some(body(slug)));
  assert_eq!(create(&owner.token, "kontur").await.status, 201);
  error(&create(&owner.token, "kontur").await, 409, ErrorCode::SlugTaken);
  for reserved in ["admin", "api", "auth"] {
    error(&create(&owner.token, reserved).await, 409, ErrorCode::SlugTaken);
  }
  for invalid in ["_hidden", ".hidden", "Kontur", "two--hyphens", ""] {
    error(&create(&owner.token, invalid).await, 422, ErrorCode::Invalid);
  }
  error(&create(&bob.token, "billing").await, 403, ErrorCode::Forbidden);
  let old_shape =
    json!({"idempotency_key":key(),"name":"A","key":"AA","remote":"https://host.example/owner/repo.git","tasks":[]});
  assert_eq!(
    server
      .request("POST", "/projects", Some(&owner.token), Some(old_shape))
      .await
      .status,
    400
  );
  let by_remote = server
    .request(
      "GET",
      "/projects?remote=host.example/owner/repo",
      Some(&owner.token),
      None,
    )
    .await;
  assert_eq!(by_remote.status, 400);
  for table in ["projects", "memberships", "project_batches"] {
    let count: i64 = server
      .database()
      .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
      .expect("count");
    assert_eq!(count, 1, "{table}");
  }
  server.finish().await;
}

#[tokio::test]
async fn s1c_the_first_checkout_linked_to_an_empty_project_keeps_its_numbers_and_later_ones_get_new() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let empty = json!({"idempotency_key":key(),"name":"Kontur","slug":"kontur","key":"ORC","tasks":[]});
  let created = server
    .request("POST", "/projects", Some(&owner.token), Some(empty))
    .await;
  let project = created.json::<ProjectCreated>().project.id;
  add_member(&server, &owner.token, project, "bob", "member").await;
  let link = format!("{}/link", path(project));
  let first =
    json!({"idempotency_key":key(),"tasks":[upload(12),upload(30)],"remote":"git@github.com:acme/kontur.git"});
  let linked = server
    .request("POST", &link, Some(&owner.token), Some(first.clone()))
    .await;
  assert_eq!(linked.status, 201);
  let linked: ProjectLinked = linked.json();
  assert_eq!(
    linked.tasks.iter().map(|task| task.number).collect::<Vec<_>>(),
    [12, 30]
  );
  let replay: ProjectLinked = server
    .request("POST", &link, Some(&owner.token), Some(first))
    .await
    .json();
  assert_eq!(replay.tasks.len(), 2);
  let mut task = upload(3);
  task["former_key"] = json!("BOB-3");
  let second = json!({"idempotency_key":key(),"tasks":[task],"remote":"https://github.com/bob/fork.git"});
  let joined: ProjectLinked = server
    .request("POST", &link, Some(&bob.token), Some(second))
    .await
    .json();
  assert_eq!(joined.tasks[0].number, 31);
  assert_eq!(joined.tasks[0].former_key.as_deref(), Some("BOB-3"));
  let shown: ProjectResponse = server
    .request("GET", &path(project), Some(&bob.token), None)
    .await
    .json();
  assert_eq!(
    shown.project.remote_hint.expect("hint").as_str(),
    "github.com/acme/kontur"
  );
  let carol = server.invited("carol", false).await;
  let outsider = json!({"idempotency_key":key(),"tasks":[]});
  assert_eq!(
    server
      .request("POST", &link, Some(&carol.token), Some(outsider))
      .await
      .status,
    404
  );
  server.finish().await;
}

#[tokio::test]
async fn s1b_membership_privacy_roles_self_leave_and_admin_management() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let admin = server.invited("admin", true).await;
  let created = project(&server, &owner.token, vec![]).await.project;
  let project = created.id;
  let lookup = server
    .request(
      "GET",
      &format!("/projects?slug={}", created.slug),
      Some(&bob.token),
      None,
    )
    .await;
  error(&lookup, 403, ErrorCode::NotMember);
  match lookup.json::<SharingError>().details {
    SharingErrorDetails::Project { name, owners } => {
      assert_eq!(name, "Shared project");
      assert_eq!(owners.iter().map(|owner| owner.id).collect::<Vec<_>>(), [owner.user.id]);
    }
    _ => panic!("project details"),
  }
  error(
    &server
      .request("GET", "/projects?slug=missing", Some(&bob.token), None)
      .await,
    404,
    ErrorCode::NotFound,
  );
  assert_eq!(
    server
      .request("GET", &path(project), Some(&admin.token), None)
      .await
      .status,
    404
  );
  let tasks = format!("{}/tasks", path(project));
  assert_eq!(
    server.request("GET", &tasks, Some(&admin.token), None).await.status,
    404
  );
  add_member(&server, &admin.token, project, "bob", "member").await;
  let shown: ProjectResponse = server
    .request("GET", &path(project), Some(&bob.token), None)
    .await
    .json();
  assert_eq!(shown.project.role, Some(orchester_api::MemberRole::Member));
  let members = format!("{}/members", path(project));
  let added = server
    .request(
      "POST",
      &members,
      Some(&bob.token),
      Some(json!({"username":"carol","role":"member"})),
    )
    .await;
  assert_eq!(added.status, 403);
  server.finish().await;
}

#[tokio::test]
async fn s1b_member_roles_last_owner_self_leave_and_error_contracts() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let carol = server.invited("carol", false).await;
  let admin = server.invited("admin", true).await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  add_member(&server, &admin.token, project, "bob", "member").await;
  let own = format!("{}/members/{}", path(project), owner.user.id);
  error(
    &server
      .request("PATCH", &own, Some(&owner.token), Some(json!({"role":"member"})))
      .await,
    409,
    ErrorCode::LastOwner,
  );
  error(
    &server.request("DELETE", &own, Some(&owner.token), None).await,
    409,
    ErrorCode::LastOwner,
  );
  let bob_path = format!("{}/members/{}", path(project), bob.user.id);
  let promoted = server
    .request("PATCH", &bob_path, Some(&admin.token), Some(json!({"role":"owner"})))
    .await;
  assert_eq!(promoted.status, 200);
  assert_eq!(
    promoted.json::<MemberResponse>().member.role,
    orchester_api::MemberRole::Owner
  );
  assert_eq!(
    server.request("DELETE", &own, Some(&owner.token), None).await.status,
    204
  );
  add_member(&server, &bob.token, project, "carol", "member").await;
  assert_eq!(
    server
      .request(
        "DELETE",
        &format!("{}/members/{}", path(project), carol.user.id),
        Some(&carol.token),
        None
      )
      .await
      .status,
    204
  );
  error(
    &server
      .request(
        "POST",
        &format!("{}/members", path(project)),
        Some(&bob.token),
        Some(json!({"username":"bob","role":"member"})),
      )
      .await,
    409,
    ErrorCode::AlreadyMember,
  );
  error(
    &server
      .request(
        "POST",
        &format!("{}/members", path(project)),
        Some(&bob.token),
        Some(json!({"username":"missing","role":"member"})),
      )
      .await,
    404,
    ErrorCode::NoSuchUser,
  );
  let count: i64 = server
    .database()
    .query_row(
      "SELECT COUNT(*) FROM audit_events WHERE action='member_role_changed'",
      [],
      |row| row.get(0),
    )
    .expect("audit");
  assert_eq!(count, 1);
  server.finish().await;
}
