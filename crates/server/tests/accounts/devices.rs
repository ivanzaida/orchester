use orchester_api::{ErrorCode, JoinedVia, ServerInfo, SessionsResponse, UserResponse};
use orchester_server::clock::{Clock, DEVICE_IDLE_MS};

use crate::support::{TestServer, credential};

#[tokio::test]
async fn s1c_setup_code_makes_the_first_admin_once_and_needs_setup_ends() {
  let server = TestServer::start(|_| {}).await;
  let info: ServerInfo = server.request("GET", "/server", None, None).await.json();
  assert_eq!(info.api_version, 3);
  assert!(info.needs_bootstrap);
  assert!(info.cloud.is_none());
  let code = server
    .server
    .host_setup_code()
    .await
    .expect("setup")
    .expect("no admin yet");
  let again = server
    .server
    .host_setup_code()
    .await
    .expect("setup")
    .expect("still no admin");
  // A new setup code ends the earlier one.
  let earlier = server.redeem(&code, Some("eve"), "laptop").await;
  assert_eq!(earlier.status, 401);
  assert_eq!(earlier.json::<orchester_api::ApiError>().error, ErrorCode::InvalidCode);
  let admin = server.join(&again, Some("alice"), "laptop").await;
  assert!(admin.user.admin && !admin.user.cloud);
  assert_eq!(admin.session.via, JoinedVia::Code);
  assert!(admin.joined.is_none());
  assert_eq!(server.redeem(&again, Some("mallory"), "laptop").await.status, 401);
  assert!(server.server.host_setup_code().await.expect("setup").is_none());
  assert!(
    !server
      .request("GET", "/server", None, None)
      .await
      .json::<ServerInfo>()
      .needs_bootstrap
  );
  let me: UserResponse = server.request("GET", "/auth/me", Some(&admin.token), None).await.json();
  assert_eq!(me.user.username, "alice");
  assert_eq!(me.user.display_name, "Alice");
  server.finish().await;
}

#[tokio::test]
async fn s1c_setup_code_needs_a_username_and_keeps_the_code_on_a_refusal() {
  let server = TestServer::start(|_| {}).await;
  let code = server
    .server
    .host_setup_code()
    .await
    .expect("setup")
    .expect("no admin yet");
  assert_eq!(server.redeem_with(&code, None, "laptop", None).await.status, 422);
  assert_eq!(server.redeem(&code, Some("not a name"), "laptop").await.status, 422);
  let blank = server
    .request(
      "POST",
      "/auth/code",
      None,
      Some(serde_json::json!({"code": code.expose(), "device": " "})),
    )
    .await;
  assert_eq!(blank.status, 422);
  server.join(&code, Some("alice"), "laptop").await;
  server.finish().await;
}

#[tokio::test]
async fn s1c_pairing_adds_devices_of_one_account_and_sessions_revoke_and_expire_after_90_idle_days() {
  let server = TestServer::start(|_| {}).await;
  let first = server.bootstrap().await;
  let second = server.another_device(&first.token).await;
  let third = server.another_device(&first.token).await;
  assert_eq!(second.user.id, first.user.id);
  let sessions: SessionsResponse = server
    .request("GET", "/auth/sessions", Some(&first.token), None)
    .await
    .json();
  assert_eq!(sessions.sessions.len(), 3);
  assert_eq!(sessions.sessions.iter().filter(|session| session.current).count(), 1);
  assert!(sessions.sessions.iter().all(|session| session.via == JoinedVia::Code));
  let path = format!("/auth/sessions/{}", third.session.id);
  assert_eq!(
    server.request("DELETE", &path, Some(&first.token), None).await.status,
    204
  );
  assert_eq!(
    server.request("GET", "/auth/me", Some(&third.token), None).await.status,
    401
  );
  assert_eq!(
    server
      .request("DELETE", "/auth/sessions/current", Some(&second.token), None)
      .await
      .status,
    204
  );
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&second.token), None)
      .await
      .status,
    401
  );
  server.clock.advance(DEVICE_IDLE_MS - 1);
  assert_eq!(
    server.request("GET", "/auth/me", Some(&first.token), None).await.status,
    200
  );
  server.clock.advance(DEVICE_IDLE_MS);
  assert_eq!(
    server.request("GET", "/auth/me", Some(&first.token), None).await.status,
    401
  );
  server.finish().await;
}

#[tokio::test]
async fn s1c_an_account_holds_three_open_pairing_codes_which_expire_after_ten_minutes() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let mut codes = Vec::new();
  for _ in 0..3 {
    let response = server.request("POST", "/auth/pairings", Some(&admin.token), None).await;
    assert_eq!(response.status, 201);
    codes.push(response.json::<orchester_api::PairingCode>());
  }
  let fourth = server.request("POST", "/auth/pairings", Some(&admin.token), None).await;
  assert_eq!(fourth.status, 429);
  assert!(fourth.headers.contains_key("retry-after"));
  assert_eq!(codes[0].expires_at, server.clock.now_ms() + 10 * 60 * 1000);
  server.join_as(&codes[0].code, "alice", "phone").await;
  server.clock.advance(10 * 60 * 1000);
  assert_eq!(server.redeem_as(&codes[1].code, "alice", "tablet").await.status, 401);
  assert_eq!(
    server
      .request("POST", "/auth/pairings", Some(&admin.token), None)
      .await
      .status,
    201
  );
  server.finish().await;
}

#[tokio::test]
async fn s1c_device_last_use_is_written_only_after_a_minute_and_expiry_keeps_its_boundary() {
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
  server.clock.advance(DEVICE_IDLE_MS - 1);
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .status,
    200
  );
  assert_eq!(last_use(), server.clock.now_ms());
  server.clock.advance(DEVICE_IDLE_MS);
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&signed.token), None)
      .await
      .status,
    401
  );
  drop(database);
  let unknown = credential("orc_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
  assert_eq!(
    server.request("GET", "/auth/me", Some(&unknown), None).await.status,
    401
  );
  server.finish().await;
}
