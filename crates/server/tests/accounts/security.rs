use std::{
  io::{self, Write},
  sync::Arc,
};

use orchester_api::{ApiError, Credential, ErrorCode, InviteCreated, Joined, PairingCode};
use parking_lot::Mutex;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::support::{TestServer, cloud::Identity, credential};

#[tokio::test]
async fn s1c_device_tokens_are_stored_as_hashes_and_no_password_remains() {
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
  let columns: Vec<String> = database
    .prepare("SELECT name FROM pragma_table_info('users')")
    .expect("columns")
    .query_map([], |row| row.get(0))
    .expect("column names")
    .collect::<Result<_, _>>()
    .expect("column list");
  assert!(!columns.iter().any(|column| column.contains("password")));
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

/// Setup, an invite code, pairing and a cloud join, with every code and token they handled.
async fn joins(server: &TestServer) -> Vec<String> {
  let setup = server.server.host_setup_code().await.expect("setup").expect("no admin");
  let admin = server.join(&setup, Some("alice"), "laptop").await;
  // The admin sends invites by email, which needs a two-factor sign-in (accounts/two_factor.rs).
  server.two_factor(&admin.token);
  let invite: InviteCreated = server
    .request(
      "POST",
      "/invites",
      Some(&admin.token),
      Some(json!({"kind":"server","delivery":"code"})),
    )
    .await
    .json();
  let code = invite.code.expect("invite code");
  let bob = server.join(&code, Some("bob"), "laptop").await;
  let pairing: PairingCode = server
    .request("POST", "/auth/pairings", Some(&bob.token), None)
    .await
    .json();
  let paired = server.join_as(&pairing.code, "bob", "phone").await;
  let cloud = server.cloud();
  let now = server.seconds();
  let key_set = cloud.key_set(now);
  let linked = server
    .request(
      "PATCH",
      "/server/cloud",
      Some(&admin.token),
      Some(json!({"link": {"root_key": cloud.root.public(), "key_set": key_set}})),
    )
    .await;
  let audience = linked.json::<orchester_api::CloudLinkResponse>().cloud.server;
  let email: InviteCreated = server
    .request(
      "POST",
      "/invites",
      Some(&admin.token),
      Some(json!({"kind":"server","delivery":"cloud","email":"carol@example.com"})),
    )
    .await
    .json();
  let invite_id = email.invite.id.to_string();
  let identity = Identity {
    audience: &audience,
    subject: "0191c7c0-0000-7000-8000-00000000ca40",
    email: "carol@example.com",
    name: "Carol",
    invite: Some(&invite_id),
  };
  let token = cloud.token(&identity, now);
  let carol: Joined = server
    .request(
      "POST",
      "/auth/cloud",
      None,
      Some(json!({"identity_token": token, "device":"laptop", "key_set": key_set})),
    )
    .await
    .json();
  let wrong = server.redeem_with(&credential("ZZZZ-ZZZZ"), None, "laptop", None).await;
  assert_eq!(wrong.status, 401);
  vec![
    setup.expose().into(),
    admin.token.expose().into(),
    code.expose().into(),
    bob.token.expose().into(),
    pairing.code.expose().into(),
    paired.token.expose().into(),
    token,
    carol.token.expose().into(),
    email.signed.expect("signed invite"),
  ]
}

#[tokio::test(flavor = "current_thread")]
async fn s1c_logs_of_setup_joins_and_pairing_hold_no_code_or_token_and_audit_is_append_only() {
  let capture = Capture(Arc::new(Mutex::new(Vec::new())));
  let writer = capture.clone();
  let subscriber = tracing_subscriber::fmt()
    .with_ansi(false)
    .with_writer(move || writer.clone())
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("install test log capture");
  let server = TestServer::start(|_| {}).await;
  let secrets = joins(&server).await;
  let admin_token = credential(&secrets[1]);
  assert_internal_diagnostics(&server, &admin_token).await;
  let secret_path = format!("/unknown/{}", secrets[1]);
  assert_eq!(server.request("GET", &secret_path, None, None).await.status, 404);
  assert_append_only_audit(&server);
  let audit: String = server
    .database()
    .query_row("SELECT group_concat(target, ' ') FROM audit_events", [], |row| {
      row.get(0)
    })
    .expect("audit targets");
  server.finish().await;
  let log = String::from_utf8(capture.0.lock().clone()).expect("log encoding");
  assert!(log.contains("request") && log.contains("status=201"));
  for secret in &secrets {
    assert!(!log.contains(secret.as_str()));
    assert!(!log.contains(&secret.replace('-', "")));
  }
  for secret in &secrets {
    assert!(!audit.contains(secret.as_str()));
  }
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
    "code_made",
    "user_created",
    "joined",
    "invite_made",
    "invite_accepted",
    "cloud_linked",
  ] {
    assert!(actions.iter().any(|found| found == action), "{action}");
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
  for (method, path) in [("POST", "/auth/code"), ("GET", "/server")] {
    let oversized = server
      .request(
        method,
        path,
        None,
        Some(json!({"code":"a".repeat(1024*1024), "device":"test"})),
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
