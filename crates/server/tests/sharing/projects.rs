use orchester_api::{
  ErrorCode, MemberResponse, ProjectCreated, ProjectResponse, Projects, SharingError, SharingErrorDetails,
};
use serde_json::json;

use crate::{fixture::*, support::TestServer};

#[tokio::test]
async fn s1b_project_upload_preserves_numbers_remote_and_first_result() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let batch = key();
  let input = json!({"idempotency_key":batch,"name":" Shared ","key":"ORC","remote":"https://user:private-remote-token@HOST.EXAMPLE:443/Owner/Repo.git/","tasks":[upload(7),upload(19)]});
  let response = server
    .request("POST", "/projects", Some(&owner.token), Some(input.clone()))
    .await;
  assert_eq!(response.status, 201);
  assert!(!response.contains("private-remote-token"));
  let created: ProjectCreated = response.json();
  assert_eq!(created.project.next_number, 20);
  assert_eq!(created.tasks.iter().map(|t| t.number).collect::<Vec<_>>(), [7, 19]);
  assert_eq!(created.project.remote.as_str(), "host.example/owner/repo");
  assert_eq!(created.tasks[0].created.at, 123);
  let changed = server
    .request(
      "PATCH",
      &path(created.project.id),
      Some(&owner.token),
      Some(json!({"key":"NEW","name":"Renamed"})),
    )
    .await;
  assert_eq!(changed.status, 200);
  let replay = server
    .request("POST", "/projects", Some(&owner.token), Some(input))
    .await;
  assert_eq!(replay.status, 201);
  assert_eq!(replay.json::<serde_json::Value>(), response.json::<serde_json::Value>());
  let listed = server
    .request(
      "GET",
      "/projects?remote=ssh%3A%2F%2Fgit%40host.example%2Fowner%2Frepo.git",
      Some(&owner.token),
      None,
    )
    .await;
  assert_eq!(listed.status, 200);
  assert_eq!(listed.json::<Projects>().projects.len(), 1);
  let created_task = create_tasks(&server, &owner.token, created.project.id, 1).await;
  assert_eq!(created_task.tasks[0].number, 20);
  assert_eq!(created_task.tasks[0].key, "NEW-20");
  let stored: String = server
    .database()
    .query_row("SELECT remote FROM projects", [], |row| row.get(0))
    .expect("stored remote");
  assert!(!stored.contains("private-remote-token"));
  server.finish().await;
}

#[tokio::test]
async fn s1b_simultaneous_project_creation_changes_only_the_winner() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let first = json!({"idempotency_key":key(),"name":"A","key":"AA","remote":"https://host.example/owner/repo.git","tasks":[upload(5)]});
  let second =
    json!({"idempotency_key":key(),"name":"B","key":"BB","remote":"git@HOST.EXAMPLE:OWNER/REPO","tasks":[upload(8)]});
  let (a, b) = tokio::join!(
    server.request("POST", "/projects", Some(&owner.token), Some(first)),
    server.request("POST", "/projects", Some(&owner.token), Some(second))
  );
  let (winner, loser) = if a.status == 201 { (a, b) } else { (b, a) };
  assert_eq!(winner.status, 201);
  error(&loser, 409, ErrorCode::RemoteTaken);
  let conflict: SharingError = loser.json();
  match conflict.details {
    SharingErrorDetails::Remote {
      project,
      member,
      owners,
    } => {
      assert_eq!(project, winner.json::<ProjectCreated>().project.id);
      assert!(member);
      assert_eq!(owners.len(), 1);
    }
    _ => panic!("remote conflict details"),
  }
  for table in ["projects", "tasks", "memberships", "project_batches"] {
    assert_eq!(
      server
        .database()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get::<_, i64>(0))
        .expect("count"),
      1
    );
  }
  server.finish().await;
}

#[tokio::test]
async fn s1b_membership_privacy_roles_self_leave_and_admin_management() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = user(&server, "bob", false).await;
  let admin = user(&server, "admin", true).await;
  let project = project(&server, &owner.token, vec![]).await.project.id;
  let lookup = server
    .request(
      "GET",
      "/projects?remote=host.example/owner/repo",
      Some(&bob.token),
      None,
    )
    .await;
  error(&lookup, 403, ErrorCode::NotMember);
  assert_eq!(
    server
      .request("GET", &path(project), Some(&admin.token), None)
      .await
      .status,
    404
  );
  assert_eq!(
    server
      .request("GET", &format!("{}/tasks", path(project)), Some(&admin.token), None)
      .await
      .status,
    404
  );
  add_member(&server, &admin.token, project, "bob", "member").await;
  assert_eq!(
    server
      .request("GET", &path(project), Some(&bob.token), None)
      .await
      .json::<ProjectResponse>()
      .project
      .role,
    orchester_api::MemberRole::Member
  );
  assert_eq!(
    server
      .request(
        "POST",
        &format!("{}/members", path(project)),
        Some(&bob.token),
        Some(json!({"username":"carol","role":"member"}))
      )
      .await
      .status,
    403
  );
  server.finish().await;
}

#[tokio::test]
async fn s1b_member_roles_last_owner_self_leave_and_error_contracts() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = user(&server, "bob", false).await;
  let carol = user(&server, "carol", false).await;
  let admin = user(&server, "admin", true).await;
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
