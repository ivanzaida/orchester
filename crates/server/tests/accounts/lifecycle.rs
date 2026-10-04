use orchester_api::{ApiError, CreateUser, ErrorCode, ServerInfo, SessionsResponse, UserResponse};
use orchester_server::clock::{Clock, SESSION_IDLE_MS};
use serde_json::json;

use crate::support::{TestServer, credential};

#[tokio::test]
async fn s1a_bootstrap_and_initial_password_restriction() {
  let server = TestServer::start(|_| {}).await;
  let info = server.request("GET", "/server", None, None).await;
  assert!(info.json::<ServerInfo>().needs_bootstrap);
  assert_eq!(info.headers["orchester-server-time"], server.clock.now_ms().to_string());
  let created = server
    .server
    .host_create_user(CreateUser {
      username: "Alice".into(),
      display_name: "Alice".into(),
      admin: true,
    })
    .await
    .expect("create admin");
  assert!(
    !server
      .request("GET", "/server", None, None)
      .await
      .json::<ServerInfo>()
      .needs_bootstrap
  );
  let signed = server.login("ALICE", &created.initial_password).await;
  assert!(signed.password_change_required);
  for path in ["/auth/me", "/auth/sessions", "/admin/users"] {
    let refused = server.request("GET", path, Some(&signed.token), None).await;
    assert_eq!(refused.status, 403);
    assert_eq!(refused.json::<ApiError>().error, ErrorCode::PasswordChangeRequired);
  }
  assert_eq!(
    server
      .request(
        "DELETE",
        &format!("/auth/sessions/{}", signed.session.id),
        Some(&signed.token),
        None
      )
      .await
      .status,
    403
  );
  let changed = server
    .request(
      "POST",
      "/auth/password",
      Some(&signed.token),
      Some(json!({"current_password": created.initial_password.expose(), "new_password": ""})),
    )
    .await;
  assert_eq!(changed.status, 422);
  let changed = server
    .request(
      "POST",
      "/auth/password",
      Some(&signed.token),
      Some(json!({"current_password": created.initial_password.expose(), "new_password": "  twelve characters  "})),
    )
    .await;
  assert_eq!(changed.status, 204);
  assert!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .json::<UserResponse>()
      .user
      .admin
  );
  server.finish().await;
}

#[tokio::test]
async fn s1a_host_reset_revokes_sessions_and_initial_session_can_sign_out() {
  let server = TestServer::start(|_| {}).await;
  let signed = server.bootstrap().await;
  let initial_again = server
    .server
    .host_reset_password("alice".into())
    .await
    .expect("host reset");
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .status,
    401
  );
  let restricted = server.login("alice", &initial_again).await;
  assert!(restricted.password_change_required);
  assert_eq!(
    server
      .request("DELETE", "/auth/sessions/current", Some(&restricted.token), None)
      .await
      .status,
    204
  );
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&restricted.token), None)
      .await
      .status,
    401
  );
  server.finish().await;
}

#[tokio::test]
async fn s1a_sessions_revocation_password_change_and_idle_expiry() {
  let server = TestServer::start(|_| {}).await;
  let first = server.bootstrap().await;
  let second = server.login("alice", &credential("a secure admin password")).await;
  let third = server.login("alice", &credential("a secure admin password")).await;
  let sessions = server
    .request("GET", "/auth/sessions", Some(&first.token), None)
    .await
    .json::<SessionsResponse>();
  assert_eq!(sessions.sessions.len(), 3);
  assert_eq!(sessions.sessions.iter().filter(|session| session.current).count(), 1);
  assert_eq!(
    server
      .request(
        "DELETE",
        &format!("/auth/sessions/{}", third.session.id),
        Some(&first.token),
        None
      )
      .await
      .status,
    204
  );
  assert_eq!(
    server.request("GET", "/auth/me", Some(&third.token), None).await.status,
    401
  );
  let wrong = server
    .request(
      "POST",
      "/auth/password",
      Some(&first.token),
      Some(json!({"current_password": "incorrect password", "new_password": "updated admin password"})),
    )
    .await;
  assert_eq!(wrong.status, 401);
  let changed = server
    .request(
      "POST",
      "/auth/password",
      Some(&first.token),
      Some(json!({"current_password": "a secure admin password", "new_password": "updated admin password"})),
    )
    .await;
  assert_eq!(changed.status, 204);
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&second.token), None)
      .await
      .status,
    401
  );
  server.clock.advance(SESSION_IDLE_MS - 1);
  assert_eq!(
    server.request("GET", "/auth/me", Some(&first.token), None).await.status,
    200
  );
  server.clock.advance(SESSION_IDLE_MS);
  assert_eq!(
    server.request("GET", "/auth/me", Some(&first.token), None).await.status,
    401
  );
  assert_eq!(server.request("GET", "/auth/me", None, None).await.status, 401);
  let renewed = server.login("alice", &credential("updated admin password")).await;
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&renewed.token), None)
      .await
      .status,
    200
  );
  server.finish().await;
}

#[tokio::test]
async fn s1a_password_character_limits_and_persistence() {
  let server = TestServer::start(|_| {}).await;
  let first = server.bootstrap().await;
  let too_long = "🦀".repeat(257);
  let response = server
    .request(
      "POST",
      "/auth/password",
      Some(&first.token),
      Some(json!({"current_password": "a secure admin password", "new_password": too_long})),
    )
    .await;
  assert_eq!(response.status, 422);
  let spaces = " ".repeat(12);
  let response = server
    .request(
      "POST",
      "/auth/password",
      Some(&first.token),
      Some(json!({"current_password": "a secure admin password", "new_password": spaces})),
    )
    .await;
  assert_eq!(response.status, 204);
  assert!(
    !server
      .login("alice", &credential(&spaces))
      .await
      .password_change_required
  );
  let unicode = "🦀".repeat(256);
  let response = server
    .request(
      "POST",
      "/auth/password",
      Some(&first.token),
      Some(json!({"current_password": spaces, "new_password": unicode})),
    )
    .await;
  assert_eq!(response.status, 204);
  let config = orchester_server::config::Config {
    data_dir: server.directory.path().into(),
    ..Default::default()
  };
  let reopened = orchester_server::Server::open(config, server.clock.clone())
    .await
    .expect("reopen persisted accounts");
  let password = reopened
    .host_reset_password("ALICE".into())
    .await
    .expect("host reset persisted user");
  let signed = server.login("alice", &password).await;
  assert!(signed.password_change_required);
  let database = server.database();
  let version: i64 = database
    .query_row("SELECT schema_version FROM meta", [], |row| row.get(0))
    .expect("schema");
  assert_eq!(version, 2);
  let mode: String = database
    .query_row("PRAGMA journal_mode", [], |row| row.get(0))
    .expect("WAL mode");
  assert_eq!(mode, "wal");
  drop(database);
  drop(reopened);
  server.finish().await;
}

#[tokio::test]
async fn s1a_session_last_use_is_written_only_after_a_minute_and_expiry_keeps_its_boundary() {
  let server = TestServer::start(|_| {}).await;
  let signed = server.bootstrap().await;
  let database = server.database();
  let last_use = || {
    database
      .query_row(
        "SELECT last_used_at FROM sessions WHERE id=?1",
        [signed.session.id.to_string()],
        |row| row.get::<_, i64>(0),
      )
      .expect("last use")
  };
  let initial = last_use();
  server.clock.advance(60_000);
  // A competing writer must not delay these reads: authentication must avoid opening a write transaction.
  database.execute_batch("BEGIN IMMEDIATE").expect("hold writer");
  for _ in 0..3 {
    assert_eq!(
      server
        .request("GET", "/auth/me", Some(&signed.token), None)
        .await
        .status,
      200
    );
  }
  database.execute_batch("ROLLBACK").expect("release writer");
  assert_eq!(last_use(), initial);
  server.clock.advance(1);
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .status,
    200
  );
  assert_eq!(last_use(), initial + 60_001);
  server.clock.advance(SESSION_IDLE_MS - 1);
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .status,
    200
  );
  assert_eq!(last_use(), server.clock.now_ms());
  server.clock.advance(SESSION_IDLE_MS);
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .status,
    401
  );
  drop(database);
  server.finish().await;
}
