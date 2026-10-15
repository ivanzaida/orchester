use orchester_api::{ApiError, Credential, ErrorCode, InviteCreated};
use orchester_server::clock::Clock;
use serde_json::{Value, json};

use crate::support::{Response, TestServer, credential};

const HOUR: i64 = 60 * 60 * 1000;

async fn invite_code(server: &TestServer, token: &Credential, body: Value) -> InviteCreated {
  let response = server.request("POST", "/invites", Some(token), Some(body)).await;
  assert_eq!(response.status, 201);
  response.json()
}

fn assert_invalid_code(response: &Response) {
  assert_eq!(response.status, 401);
  assert_eq!(response.json::<ApiError>().error, ErrorCode::InvalidCode);
}

#[tokio::test]
async fn s1c_codes_are_one_time_case_and_hyphen_blind_and_expire_by_kind() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let invite = invite_code(&server, &admin.token, json!({"kind":"server","delivery":"code"})).await;
  let code = invite.code.expect("a code invite returns its code");
  assert_eq!(invite.invite.expires_at, server.clock.now_ms() + 72 * HOUR);
  assert!(invite.signed.is_none());
  let text = code.expose();
  assert_eq!(text.len(), 9);
  assert_eq!(&text[4..5], "-");
  // Typed without the hyphen and in lower case, the code still works, once.
  let typed = credential(&text.replace('-', "").to_lowercase());
  let bob = server.join(&typed, Some("bob"), "laptop").await;
  assert!(!bob.user.admin);
  assert_invalid_code(&server.redeem(&code, Some("bobby"), "laptop").await);
  // An invite code lives 72 hours.
  let late = invite_code(&server, &admin.token, json!({"kind":"server","delivery":"code"})).await;
  server.clock.advance(72 * HOUR - 1);
  let carol = server
    .redeem(late.code.as_ref().expect("code"), Some("carol"), "laptop")
    .await;
  assert_eq!(carol.status, 201);
  let expired = invite_code(&server, &admin.token, json!({"kind":"server","delivery":"code"})).await;
  server.clock.advance(72 * HOUR);
  assert_invalid_code(
    &server
      .redeem(expired.code.as_ref().expect("code"), Some("dave"), "laptop")
      .await,
  );
  // An account code for a person who lost every device lives 24 hours and gives their own account.
  let account = server
    .server
    .host_account_code("bob".into())
    .await
    .expect("account code");
  server.clock.advance(24 * HOUR - 1);
  let again = server.join(&account, None, "new laptop").await;
  assert_eq!(again.user.id, bob.user.id);
  let account = server
    .server
    .host_account_code("BOB".into())
    .await
    .expect("account code");
  server.clock.advance(24 * HOUR);
  assert_invalid_code(&server.redeem(&account, None, "new laptop").await);
  assert!(server.server.host_account_code("nobody".into()).await.is_err());
  server.finish().await;
}

#[tokio::test]
async fn s1c_unknown_used_and_malformed_codes_answer_alike_and_codes_are_stored_only_as_hmac() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let invite = invite_code(&server, &admin.token, json!({"kind":"server","delivery":"code"})).await;
  let code = invite.code.expect("code");
  let unknown = server.redeem(&credential("0000-0000"), Some("x"), "laptop").await;
  let malformed = server.redeem(&credential("not a code"), Some("x"), "laptop").await;
  assert_invalid_code(&unknown);
  assert_invalid_code(&malformed);
  assert!(!unknown.contains("0000") && !malformed.contains("not a code"));
  let bytes = std::fs::read(server.directory.path().join("orchester-server.db")).expect("database bytes");
  let wal = std::fs::read(server.directory.path().join("orchester-server.db-wal")).unwrap_or_default();
  let plain = code.expose().replace('-', "");
  for haystack in [&bytes, &wal] {
    assert!(!haystack.windows(plain.len()).any(|window| window == plain.as_bytes()));
    assert!(
      !haystack
        .windows(code.expose().len())
        .any(|window| window == code.expose().as_bytes())
    );
  }
  let stored: Vec<u8> = server
    .database()
    .query_row("SELECT hmac FROM codes WHERE kind='invite'", [], |row| row.get(0))
    .expect("stored HMAC");
  assert_eq!(stored.len(), 32);
  server.finish().await;
}

#[tokio::test]
async fn s1c_failed_redemptions_block_an_address_after_ten() {
  let server = TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.1".parse().expect("proxy")]).await;
  let admin = server.bootstrap().await;
  let wrong = credential("ZZZZ-ZZZZ");
  let from = |address: &str| vec![("X-Forwarded-For", address.to_owned())];
  let redeem = |address: String| {
    let wrong = &wrong;
    let server = &server;
    async move {
      let headers = from(&address);
      let headers: Vec<(&str, &str)> = headers.iter().map(|(key, value)| (*key, value.as_str())).collect();
      server
        .request_headers(
          "POST",
          "/auth/code",
          None,
          Some(json!({"code": wrong.expose(), "device":"x"})),
          &headers,
        )
        .await
    }
  };
  for _ in 0..10 {
    assert_eq!(redeem("198.51.100.1".into()).await.status, 401);
  }
  let blocked = redeem("198.51.100.1".into()).await;
  assert_eq!(blocked.status, 429);
  assert_eq!(blocked.headers["retry-after"], "900");
  // A valid code from the blocked address waits too; another address is not affected.
  let invite = invite_code(&server, &admin.token, json!({"kind":"server","delivery":"code"})).await;
  let headers = [("X-Forwarded-For", "198.51.100.1")];
  let body =
    json!({"code": invite.code.as_ref().expect("code").expose(), "device":"x", "username":"bob", "display_name":"Bob"});
  assert_eq!(
    server
      .request_headers("POST", "/auth/code", None, Some(body.clone()), &headers)
      .await
      .status,
    429
  );
  server.clock.advance(15 * 60 * 1000);
  assert_eq!(
    server
      .request_headers("POST", "/auth/code", None, Some(body), &headers)
      .await
      .status,
    201
  );
  assert_eq!(redeem("203.0.113.1".into()).await.status, 401);
  server.finish().await;
}

#[tokio::test]
async fn s1c_a_server_holds_fifty_open_codes() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  for _ in 0..50 {
    invite_code(&server, &admin.token, json!({"kind":"server","delivery":"code"})).await;
  }
  let refused = server
    .request(
      "POST",
      "/invites",
      Some(&admin.token),
      Some(json!({"kind":"server","delivery":"code"})),
    )
    .await;
  assert_eq!(refused.status, 409);
  assert_eq!(refused.json::<ApiError>().error, ErrorCode::TooManyCodes);
  // A pairing code is refused the same way: waiting a minute would not help.
  let pairing = server.request("POST", "/auth/pairings", Some(&admin.token), None).await;
  assert_eq!(pairing.status, 409);
  assert_eq!(pairing.json::<ApiError>().error, ErrorCode::TooManyCodes);
  server.clock.advance(72 * HOUR);
  invite_code(&server, &admin.token, json!({"kind":"server","delivery":"code"})).await;
  server.finish().await;
}

#[tokio::test]
async fn s1c_one_code_joins_once_however_many_redeem_it_at_once() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let created = invite_code(&server, &admin.token, json!({"kind":"server","delivery":"code"})).await;
  let code = created.code.as_ref().expect("code");
  let (a, b, c, d) = tokio::join!(
    server.redeem(code, Some("bob"), "laptop"),
    server.redeem(code, Some("carol"), "laptop"),
    server.redeem(code, Some("dave"), "laptop"),
    server.redeem(code, Some("erin"), "laptop"),
  );
  let statuses = [a.status, b.status, c.status, d.status];
  assert_eq!(
    statuses.iter().filter(|status| **status == 201).count(),
    1,
    "{statuses:?}"
  );
  assert_eq!(
    statuses.iter().filter(|status| **status == 401).count(),
    3,
    "{statuses:?}"
  );
  let accounts: i64 = server
    .database()
    .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
    .expect("accounts");
  assert_eq!(accounts, 2);
  server.finish().await;
}
