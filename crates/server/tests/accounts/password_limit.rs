use orchester_api::{ApiError, CreateUser, Credential, ErrorCode};
use serde_json::json;

use crate::support::{Response, TestServer, credential};

async fn change(server: &TestServer, token: &Credential, current: &str) -> Response {
  server
    .request(
      "POST",
      "/auth/password",
      Some(token),
      Some(json!({"current_password": current, "new_password": "updated test password"})),
    )
    .await
}

#[tokio::test]
async fn s1a_password_changes_limit_per_user_audit_failures_and_skip_blocked_hash_work() {
  let server = TestServer::start(|_| {}).await;
  let first = server.bootstrap().await;
  let second = server.login("alice", &credential("a secure admin password")).await;
  let bob = server
    .server
    .host_create_user(CreateUser {
      username: "bob".into(),
      display_name: "Bob".into(),
      admin: false,
    })
    .await
    .expect("second user");
  let other = server.login("bob", &bob.initial_password).await;
  for index in 0..5 {
    let token = if index % 2 == 0 { &first.token } else { &second.token };
    assert_eq!(change(&server, token, "wrong current password").await.status, 401);
  }
  let database = server.database();
  let rows: i64 = database.query_row(
    "SELECT COUNT(*) FROM audit_events WHERE action='password_change_failed' AND actor=?1 AND target=?1 AND address='127.0.0.1'",
    [first.user.id.to_string()], |row| row.get(0)).expect("failure audit");
  assert_eq!(rows, 5);
  let hash: String = database
    .query_row("SELECT password_hash FROM users WHERE username='alice'", [], |row| {
      row.get(0)
    })
    .expect("password hash");
  // A blocked request must return before parsing a hash, let alone acquiring a slot or doing Argon2 work.
  database
    .execute("UPDATE users SET password_hash='malformed' WHERE username='alice'", [])
    .expect("hash probe");
  let blocked = change(&server, &second.token, "a secure admin password").await;
  assert_eq!(blocked.status, 429);
  assert_eq!(blocked.json::<ApiError>().error, ErrorCode::RateLimited);
  assert_eq!(blocked.headers["retry-after"], "900");
  database
    .execute("UPDATE users SET password_hash=?1 WHERE username='alice'", [&hash])
    .expect("restore hash");
  assert!(
    !server
      .login("alice", &credential("a secure admin password"))
      .await
      .password_change_required
  );
  assert_eq!(
    change(&server, &other.token, "wrong current password").await.status,
    401
  );
  server.clock.advance(15 * 60 * 1000 - 1000);
  assert_eq!(
    change(&server, &first.token, "a secure admin password").await.headers["retry-after"],
    "1"
  );
  server.clock.advance(1000);
  assert_eq!(
    change(&server, &first.token, "a secure admin password").await.status,
    204
  );
  let rows: i64 = database
    .query_row(
      "SELECT COUNT(*) FROM audit_events WHERE action='password_change_failed' AND actor=?1",
      [first.user.id.to_string()],
      |row| row.get(0),
    )
    .expect("failure audit count");
  assert_eq!(rows, 5);
  let leaked: bool = database.query_row(
    "SELECT EXISTS(SELECT 1 FROM audit_events WHERE actor='wrong current password' OR target='wrong current password' OR address='wrong current password')",
    [], |row| row.get(0)).expect("audit redaction");
  assert!(!leaked);
  drop(database);
  server.finish().await;
}
