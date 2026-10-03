use orchester_api::{ApiError, Credential, ErrorCode, PasswordReset, SignedIn, UserCreated, UsersResponse};
use serde_json::json;

use crate::support::TestServer;

async fn create_bob(server: &TestServer, admin: &Credential) -> UserCreated {
  let response = server
    .request(
      "POST",
      "/admin/users",
      Some(admin),
      Some(json!({"username": "bob", "display_name": "Bob", "admin": false})),
    )
    .await;
  assert_eq!(response.status, 201);
  response.json()
}

async fn sign_in_bob(server: &TestServer, bob: &UserCreated) -> SignedIn {
  let signed = server.login("bob", &bob.initial_password).await;
  let changed = server
    .request(
      "POST",
      "/auth/password",
      Some(&signed.token),
      Some(json!({"current_password": bob.initial_password.expose(), "new_password": "secure bob password"})),
    )
    .await;
  assert_eq!(changed.status, 204);
  signed
}

#[tokio::test]
async fn s1a_nonadmin_cannot_use_admin_endpoints_or_revoke_another_users_session() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = create_bob(&server, &admin.token).await;
  let signed = sign_in_bob(&server, &bob).await;
  let path = format!("/admin/users/{}", bob.user.id);
  for (method, path, body) in [
    ("GET", "/admin/users".into(), None),
    (
      "POST",
      "/admin/users".into(),
      Some(json!({"username":"charlie", "display_name":"Charlie", "admin":false})),
    ),
    (
      "POST",
      "/admin/users".into(),
      Some(json!({"username":"", "display_name":"", "admin":false})),
    ),
    ("PATCH", path.clone(), Some(json!({"display_name":"Robert"}))),
    ("POST", format!("{path}/password"), None),
    ("DELETE", format!("{path}/sessions"), None),
  ] {
    assert_eq!(
      server.request(method, &path, Some(&signed.token), body).await.status,
      403
    );
  }
  assert_eq!(
    server
      .request(
        "DELETE",
        &format!("/auth/sessions/{}", admin.session.id),
        Some(&signed.token),
        None
      )
      .await
      .status,
    404
  );
  server.finish().await;
}

#[tokio::test]
async fn s1a_admin_creates_lists_updates_and_revokes_users() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = create_bob(&server, &admin.token).await;
  let signed = sign_in_bob(&server, &bob).await;
  let path = format!("/admin/users/{}", bob.user.id);
  let duplicate = server
    .request(
      "POST",
      "/admin/users",
      Some(&admin.token),
      Some(json!({"username":"BOB", "display_name":"Duplicate", "admin":false})),
    )
    .await;
  assert_eq!(duplicate.status, 409);
  assert_eq!(duplicate.json::<ApiError>().error, ErrorCode::UsernameTaken);
  let users = server
    .request("GET", "/admin/users", Some(&admin.token), None)
    .await
    .json::<UsersResponse>();
  assert_eq!(users.users.len(), 2);
  assert_eq!(
    server
      .request(
        "PATCH",
        &path,
        Some(&admin.token),
        Some(json!({"display_name":"Robert"}))
      )
      .await
      .status,
    200
  );
  assert_eq!(
    server
      .request("DELETE", &format!("{path}/sessions"), Some(&admin.token), None)
      .await
      .status,
    204
  );
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .status,
    401
  );
  server.finish().await;
}

#[tokio::test]
async fn s1a_admin_reset_disable_and_enable() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = create_bob(&server, &admin.token).await;
  let signed = sign_in_bob(&server, &bob).await;
  let path = format!("/admin/users/{}", bob.user.id);
  let reset = server
    .request("POST", &format!("{path}/password"), Some(&admin.token), None)
    .await
    .json::<PasswordReset>();
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .status,
    401
  );
  let restricted = server.login("bob", &reset.initial_password).await;
  assert!(restricted.password_change_required);
  assert_eq!(
    server
      .request("PATCH", &path, Some(&admin.token), Some(json!({"disabled":true})))
      .await
      .status,
    200
  );
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&restricted.token), None)
      .await
      .status,
    401
  );
  let denied = server
    .request(
      "POST",
      "/auth/sessions",
      None,
      Some(json!({"username":"bob", "password":reset.initial_password.expose(), "device":"disabled"})),
    )
    .await;
  assert_eq!(denied.status, 401);
  assert_eq!(
    server
      .request("PATCH", &path, Some(&admin.token), Some(json!({"disabled":false})))
      .await
      .status,
    200
  );
  assert!(
    server
      .login("bob", &reset.initial_password)
      .await
      .password_change_required
  );
  let database = server.database();
  for action in ["user_changed", "password_reset", "session_revoked"] {
    let found: bool = database
      .query_row(
        "SELECT EXISTS(SELECT 1 FROM audit_events WHERE action=?1)",
        [action],
        |row| row.get(0),
      )
      .expect("audit record");
    assert!(found);
  }
  drop(database);
  server.finish().await;
}

#[tokio::test]
async fn s1a_last_admin_is_protected_even_for_concurrent_demotions() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let path = format!("/admin/users/{}", admin.user.id);
  for change in [json!({"disabled":true}), json!({"admin":false})] {
    let response = server.request("PATCH", &path, Some(&admin.token), Some(change)).await;
    assert_eq!(response.status, 409);
    assert_eq!(response.json::<ApiError>().error, ErrorCode::LastAdmin);
  }
  let second = server
    .request(
      "POST",
      "/admin/users",
      Some(&admin.token),
      Some(json!({"username":"second", "display_name":"Second", "admin":true})),
    )
    .await
    .json::<UserCreated>();
  let second_path = format!("/admin/users/{}", second.user.id);
  let (first, second) = tokio::join!(
    server.request("PATCH", &path, Some(&admin.token), Some(json!({"admin":false}))),
    server.request("PATCH", &second_path, Some(&admin.token), Some(json!({"admin":false})))
  );
  assert!(first.status == 200 || second.status == 200);
  let admins: i64 = server
    .database()
    .query_row(
      "SELECT COUNT(*) FROM users WHERE admin=1 AND disabled_at IS NULL",
      [],
      |row| row.get(0),
    )
    .expect("admin count");
  assert_eq!(admins, 1);
  server.finish().await;
}
