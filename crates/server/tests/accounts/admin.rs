use orchester_api::{ApiError, ErrorCode, User, UserResponse, UsersResponse};
use serde_json::json;

use crate::support::TestServer;

#[tokio::test]
async fn s1c_nonadmin_cannot_use_admin_endpoints_or_revoke_another_users_session() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  for (method, path) in [
    ("GET", "/admin/users".to_owned()),
    ("PATCH", format!("/admin/users/{}", admin.user.id)),
    ("DELETE", format!("/admin/users/{}/sessions", admin.user.id)),
    ("GET", "/server/settings".into()),
    ("GET", "/server/cloud".into()),
  ] {
    let body = (method == "PATCH").then(|| json!({"display_name":"Bob"}));
    let response = server.request(method, &path, Some(&bob.token), body).await;
    assert_eq!(response.status, 403, "{method} {path}");
    assert_eq!(response.json::<ApiError>().error, ErrorCode::Forbidden);
  }
  let path = format!("/auth/sessions/{}", admin.session.id);
  assert_eq!(
    server.request("DELETE", &path, Some(&bob.token), None).await.status,
    404
  );
  let invite = server
    .request(
      "POST",
      "/invites",
      Some(&bob.token),
      Some(json!({"kind":"server","delivery":"code"})),
    )
    .await;
  assert_eq!(invite.status, 403);
  assert_eq!(
    server.request("GET", "/auth/me", Some(&admin.token), None).await.status,
    200
  );
  server.finish().await;
}

#[tokio::test]
async fn s1c_admin_lists_updates_disables_and_revokes_users() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let bob_again = server.another_device(&bob.token).await;
  let users: UsersResponse = server
    .request("GET", "/admin/users", Some(&admin.token), None)
    .await
    .json();
  let names: Vec<_> = users.users.iter().map(|user| user.username.as_str()).collect();
  assert_eq!(names, ["alice", "bob"]);
  let path = format!("/admin/users/{}", bob.user.id);
  let renamed: UserResponse = server
    .request(
      "PATCH",
      &path,
      Some(&admin.token),
      Some(json!({"display_name":"Bob Smith","admin":true})),
    )
    .await
    .json();
  assert!(renamed.user.admin);
  assert_eq!(renamed.user.display_name, "Bob Smith");
  let sessions = format!("/admin/users/{}/sessions", bob.user.id);
  assert_eq!(
    server
      .request("DELETE", &sessions, Some(&admin.token), None)
      .await
      .status,
    204
  );
  assert_eq!(
    server.request("GET", "/auth/me", Some(&bob.token), None).await.status,
    401
  );
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&bob_again.token), None)
      .await
      .status,
    401
  );
  let disabled: User = server
    .request("PATCH", &path, Some(&admin.token), Some(json!({"disabled":true})))
    .await
    .json::<UserResponse>()
    .user;
  assert!(disabled.disabled);
  let missing = format!("/admin/users/{}/sessions", orchester_core::id::UserId::generate());
  assert_eq!(
    server
      .request("DELETE", &missing, Some(&admin.token), None)
      .await
      .status,
    404
  );
  server.finish().await;
}

#[tokio::test]
async fn s1c_last_admin_is_protected() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let path = format!("/admin/users/{}", admin.user.id);
  for change in [json!({"admin":false}), json!({"disabled":true})] {
    let response = server.request("PATCH", &path, Some(&admin.token), Some(change)).await;
    assert_eq!(response.status, 409);
    assert_eq!(response.json::<ApiError>().error, ErrorCode::LastAdmin);
  }
  server.finish().await;
}

#[tokio::test]
async fn s1c_password_endpoints_are_gone() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let sign_in = server
    .request(
      "POST",
      "/auth/sessions",
      None,
      Some(json!({"username":"alice","password":"a secure admin password","device":"test"})),
    )
    .await;
  assert_eq!(sign_in.status, 405);
  let password = server
    .request(
      "POST",
      "/auth/password",
      Some(&admin.token),
      Some(json!({"current_password":"a","new_password":"b"})),
    )
    .await;
  assert_eq!(password.status, 404);
  let create = server
    .request(
      "POST",
      "/admin/users",
      Some(&admin.token),
      Some(json!({"username":"bob","display_name":"Bob","admin":false})),
    )
    .await;
  assert_eq!(create.status, 405);
  let reset = format!("/admin/users/{}/password", admin.user.id);
  assert_eq!(
    server.request("POST", &reset, Some(&admin.token), None).await.status,
    404
  );
  server.finish().await;
}
