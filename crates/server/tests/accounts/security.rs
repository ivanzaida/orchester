use std::{
  io::{self, Write},
  sync::Arc,
  time::Instant,
};

use orchester_api::{ApiError, CreateUser, Credential, ErrorCode, UserCreated};
use parking_lot::Mutex;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::support::{TestServer, credential};

async fn failed(server: &TestServer, username: &str, headers: &[(&str, &str)]) -> crate::support::Response {
  server
    .request_headers(
      "POST",
      "/auth/sessions",
      None,
      Some(json!({"username": username, "password":"a wrong test password", "device":"test"})),
      headers,
    )
    .await
}

#[tokio::test]
async fn s1a_username_rate_limit_and_unknown_user_verification() {
  let server = TestServer::start(|_| {}).await;
  server.bootstrap().await;
  let mut known_time = std::time::Duration::ZERO;
  let mut unknown_time = std::time::Duration::ZERO;
  for _ in 0..3 {
    let start = Instant::now();
    let known = failed(&server, "ALICE", &[]).await;
    known_time += start.elapsed();
    let start = Instant::now();
    let unknown = failed(&server, "missing", &[]).await;
    unknown_time += start.elapsed();
    assert_eq!(known.status, 401);
    assert_eq!(unknown.status, 401);
    assert_eq!(known.json::<ApiError>().error, unknown.json::<ApiError>().error);
  }
  // Broad scheduling tolerance still catches skipping Argon2 for unknown users.
  assert!(unknown_time.as_secs_f64() > known_time.as_secs_f64() / 4.0);
  assert!(unknown_time.as_secs_f64() < known_time.as_secs_f64() * 4.0);
  for _ in 0..2 {
    assert_eq!(failed(&server, "alice", &[]).await.status, 401);
  }
  let limited = failed(&server, "Alice", &[]).await;
  assert_eq!(limited.status, 429);
  assert_eq!(limited.headers["retry-after"], "900");
  assert_eq!(limited.json::<ApiError>().error, ErrorCode::RateLimited);
  server.clock.advance(15 * 60 * 1000 - 1000);
  assert_eq!(failed(&server, "alice", &[]).await.headers["retry-after"], "1");
  server.clock.advance(1000);
  assert_eq!(failed(&server, "alice", &[]).await.status, 401);
  server.finish().await;
}

#[tokio::test]
async fn s1a_address_rate_limit_ignores_untrusted_forwarding() {
  let server = TestServer::start(|_| {}).await;
  for index in 0..20 {
    let spoof = format!("192.0.2.{}", index + 1);
    assert_eq!(
      failed(&server, &format!("missing{index}"), &[("X-Forwarded-For", &spoof)])
        .await
        .status,
      401
    );
  }
  let limited = failed(&server, "another", &[("X-Forwarded-For", "198.51.100.1")]).await;
  assert_eq!(limited.status, 429);
  assert_eq!(limited.headers["retry-after"], "900");
  server.finish().await;
}

#[tokio::test]
async fn s1a_trusted_proxy_uses_the_appended_hop_and_separates_clients() {
  let server = TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.1".parse().expect("proxy")]).await;
  for index in 0..20 {
    let header = format!("192.0.2.{}, 198.51.100.7", index + 1);
    assert_eq!(
      failed(&server, &format!("unknown{index}"), &[("X-Forwarded-For", &header)])
        .await
        .status,
      401
    );
  }
  assert_eq!(
    failed(&server, "one_more", &[("X-Forwarded-For", "192.0.2.99, 198.51.100.7")])
      .await
      .status,
    429
  );
  assert_eq!(
    failed(&server, "other_client", &[("X-Forwarded-For", "198.51.100.8")])
      .await
      .status,
    401
  );
  server.finish().await;
}

#[tokio::test]
async fn s1a_token_storage_and_password_parameters() {
  let server = TestServer::start(|_| {}).await;
  let signed = server.bootstrap().await;
  assert!(signed.token.expose().starts_with("orc_"));
  assert_eq!(signed.token.expose().len(), 68);
  assert!(signed.token.expose()[4..].bytes().all(|byte| byte.is_ascii_hexdigit()));
  let database = server.database();
  let stored: Vec<u8> = database
    .query_row(
      "SELECT token_hash FROM sessions WHERE id=?1",
      [signed.session.id.to_string()],
      |row| row.get(0),
    )
    .expect("token digest");
  assert!(stored == Sha256::digest(signed.token.expose().as_bytes()).as_slice());
  let password: String = database
    .query_row("SELECT password_hash FROM users", [], |row| row.get(0))
    .expect("PHC hash");
  assert!(password.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
  let bytes = std::fs::read(server.directory.path().join("orchester-server.db")).expect("database bytes");
  assert!(
    !bytes
      .windows(signed.token.expose().len())
      .any(|window| window == signed.token.expose().as_bytes())
  );
  drop(database);
  server.finish().await;
}

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.0.lock().extend_from_slice(bytes);
    Ok(bytes.len())
  }
  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

#[tokio::test(flavor = "current_thread")]
async fn s1a_logs_errors_and_debug_redact_credentials_and_audit_is_append_only() {
  let capture = Capture(Arc::new(Mutex::new(Vec::new())));
  let writer = capture.clone();
  let subscriber = tracing_subscriber::fmt()
    .with_ansi(false)
    .with_writer(move || writer.clone())
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("install test log capture");
  let server = TestServer::start(|_| {}).await;
  let bootstrap = server
    .server
    .host_create_user(CreateUser {
      username: "alice".into(),
      display_name: "Alice".into(),
      admin: true,
    })
    .await
    .expect("bootstrap");
  let signed = server.login("alice", &bootstrap.initial_password).await;
  let changed = server
    .request(
      "POST",
      "/auth/password",
      Some(&signed.token),
      Some(json!({"current_password": bootstrap.initial_password.expose(), "new_password": "a secure admin password"})),
    )
    .await;
  assert_eq!(changed.status, 204);
  let created = server
    .request(
      "POST",
      "/admin/users",
      Some(&signed.token),
      Some(json!({"username":"bob", "display_name":"Bob", "admin":false})),
    )
    .await
    .json::<UserCreated>();
  assert_eq!(failed(&server, "bob", &[]).await.status, 401);
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .status,
    200
  );
  let malformed = server
    .request(
      "POST",
      "/auth/password",
      Some(&signed.token),
      Some(json!({"current_password": [signed.token.expose()], "new_password": created.initial_password.expose()})),
    )
    .await;
  assert_eq!(malformed.status, 400);
  assert!(!malformed.contains(signed.token.expose()));
  assert_internal_diagnostics(&server, &signed.token).await;
  let secret_path = format!("/unknown/{}", signed.token.expose());
  assert_eq!(server.request("GET", &secret_path, None, None).await.status, 404);
  let debug = format!("{signed:?} {created:?} {:?}", credential("a secure admin password"));
  let initial = created.initial_password.expose();
  assert!(!debug.contains(initial));
  assert!(!debug.contains(signed.token.expose()));
  assert!(!debug.contains("a secure admin password"));
  assert_eq!(
    server
      .request("DELETE", "/auth/sessions/current", Some(&signed.token), None)
      .await
      .status,
    204
  );
  assert_append_only_audit(&server);
  server.finish().await;
  let log = String::from_utf8(capture.0.lock().clone()).expect("log encoding");
  assert!(log.contains("request") && log.contains("status=201"));
  assert!(!log.contains(bootstrap.initial_password.expose()));
  assert!(!log.contains(signed.token.expose()));
  assert!(!log.contains(initial));
  assert!(!log.contains("a secure admin password"));
  assert!(!log.contains("a wrong test password"));
  assert!(log.contains("error_kind=\"Database\"") && log.contains("database_variant=\"SqliteFailure\""));
  assert!(log.contains("ConstraintViolation") && log.contains("sqlite_extended_code=1811"));
}

async fn assert_internal_diagnostics(server: &TestServer, token: &Credential) {
  let database = server.database();
  database
    .execute_batch(
      "CREATE TRIGGER diagnostic_canary BEFORE UPDATE ON users BEGIN SELECT RAISE(ABORT, NEW.display_name); END;",
    )
    .expect("error fixture");
  let user: String = database
    .query_row("SELECT id FROM users WHERE username='alice'", [], |row| row.get(0))
    .expect("admin id");
  let response = server
    .request(
      "PATCH",
      &format!("/admin/users/{user}"),
      Some(token),
      Some(json!({"display_name": token.expose()})),
    )
    .await;
  assert_eq!(response.status, 500);
  assert_eq!(response.json::<ApiError>().error, ErrorCode::Internal);
  assert!(!response.contains(token.expose()));
  database
    .execute_batch("DROP TRIGGER diagnostic_canary")
    .expect("remove error fixture");
  let error = orchester_server::error::ServerError::Database(rusqlite::Error::SqliteFailure(
    rusqlite::ffi::Error::new(19),
    Some(token.expose().into()),
  ));
  assert!(!format!("{error:?} {error}").contains(token.expose()));
}

fn assert_append_only_audit(server: &TestServer) {
  let database = server.database();
  let actions = database
    .prepare("SELECT action FROM audit_events")
    .expect("audit query")
    .query_map([], |row| row.get::<_, String>(0))
    .expect("audit rows")
    .collect::<Result<Vec<_>, _>>()
    .expect("audit actions");
  for action in [
    "user_created",
    "signed_in",
    "password_changed",
    "sign_in_failed",
    "session_revoked",
  ] {
    assert!(actions.iter().any(|found| found == action));
  }
  assert!(database.execute("DELETE FROM audit_events", []).is_err());
  assert!(database.execute("UPDATE audit_events SET action='edited'", []).is_err());
}

#[tokio::test]
async fn s1a_request_limits_and_unmatched_paths_do_not_echo_secrets() {
  let server = TestServer::start(|_| {}).await;
  let secret = credential("orc_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
  let path = format!("/unknown/{}", secret.expose());
  let response = server.request("GET", &path, None, None).await;
  assert_eq!(response.status, 404);
  assert!(!response.contains(secret.expose()));
  let response = server
    .request("GET", "/auth/me", Some(&Credential::new("invalid".into())), None)
    .await;
  assert_eq!(response.status, 401);
  for (method, path) in [("POST", "/auth/sessions"), ("GET", "/server")] {
    let oversized = server
      .request(
        method,
        path,
        None,
        Some(json!({"username":"alice", "password":"a".repeat(1024*1024), "device":"test"})),
      )
      .await;
    assert_eq!(oversized.status, 413);
    assert_eq!(oversized.json::<ApiError>().error, ErrorCode::PayloadTooLarge);
  }
  let wrong_method = server.request("POST", "/server", None, None).await;
  assert_eq!(wrong_method.status, 405);
  assert_eq!(wrong_method.json::<ApiError>().error, ErrorCode::MethodNotAllowed);
  assert!(!response.headers.contains_key("access-control-allow-origin"));
  assert_eq!(response.headers["cache-control"], "no-store");
  server.finish().await;
}
