use std::sync::Arc;

use orchester_api::{ApiError, ErrorCode, SignedIn};
use serde_json::json;
use tokio::{sync::Barrier, task::JoinSet};

use crate::support::{Response, TestServer, credential};

async fn burst(
  server: &Arc<TestServer>,
  signed: &Arc<SignedIn>,
  password_change: bool,
  different_users: bool,
) -> Vec<Response> {
  let ready = Arc::new(Barrier::new(41));
  let mut tasks = JoinSet::new();
  for index in 0..40 {
    let server = server.clone();
    let signed = signed.clone();
    let ready = ready.clone();
    tasks.spawn(async move {
      ready.wait().await;
      if password_change {
        server
          .request(
            "POST",
            "/auth/password",
            Some(&signed.token),
            Some(json!({
              "current_password": "wrong current password", "new_password": "updated test password"
            })),
          )
          .await
      } else {
        let username = if different_users {
          format!("unknown{index}")
        } else {
          "ALICE".into()
        };
        server
          .request(
            "POST",
            "/auth/sessions",
            None,
            Some(json!({
              "username": username, "password": "wrong sign-in password", "device": "test"
            })),
          )
          .await
      }
    });
  }
  ready.wait().await;
  let mut responses = Vec::new();
  while let Some(response) = tasks.join_next().await {
    responses.push(response.expect("burst worker"));
  }
  responses
}

fn assert_burst(responses: &[Response], admitted_status: u16, admitted: usize) {
  assert_eq!(
    responses
      .iter()
      .filter(|response| response.status == admitted_status)
      .count(),
    admitted
  );
  assert_eq!(
    responses.iter().filter(|response| response.status == 429).count(),
    40 - admitted
  );
  for response in responses.iter().filter(|response| response.status == 429) {
    assert_eq!(response.json::<ApiError>().error, ErrorCode::RateLimited);
    assert!(response.headers["retry-after"].parse::<u64>().expect("retry seconds") > 0);
  }
}

async fn finish(server: Arc<TestServer>) {
  Arc::try_unwrap(server)
    .unwrap_or_else(|_| panic!("burst released fixture references"))
    .finish()
    .await;
}

#[tokio::test]
async fn s1a_concurrent_password_guesses_reserve_five_attempts_before_hashing() {
  let server = Arc::new(TestServer::start(|_| {}).await);
  let signed = Arc::new(server.bootstrap().await);
  assert_burst(&burst(&server, &signed, true, false).await, 401, 5);
  let failed: i64 = server
    .database()
    .query_row(
      "SELECT COUNT(*) FROM audit_events WHERE action='password_change_failed'",
      [],
      |row| row.get(0),
    )
    .expect("failure audit count");
  assert_eq!(failed, 5);
  // The user counter must expire and sign-in must remain independent.
  server.login("alice", &credential("a secure admin password")).await;
  server.clock.advance(15 * 60 * 1000);
  assert_burst(&burst(&server, &signed, true, false).await, 401, 5);
  finish(server).await;
}

#[tokio::test]
async fn s1a_concurrent_sign_in_reserves_username_and_address_budgets() {
  for different_users in [false, true] {
    let server = Arc::new(TestServer::start(|_| {}).await);
    let signed = Arc::new(server.bootstrap().await);
    let admitted = if different_users { 20 } else { 5 };
    assert_burst(&burst(&server, &signed, false, different_users).await, 401, admitted);
    let failed: i64 = server
      .database()
      .query_row(
        "SELECT COUNT(*) FROM audit_events WHERE action='sign_in_failed'",
        [],
        |row| row.get(0),
      )
      .expect("sign-in failure count");
    assert_eq!(failed as usize, admitted);
    finish(server).await;
  }
}

#[tokio::test]
async fn s1a_rejected_concurrent_requests_skip_the_verifier_and_success_releases_reservations() {
  for password_change in [false, true] {
    let server = Arc::new(TestServer::start(|_| {}).await);
    let signed = Arc::new(server.bootstrap().await);
    for _ in 0..6 {
      server.login("alice", &credential("a secure admin password")).await;
      let response = server
        .request(
          "POST",
          "/auth/password",
          Some(&signed.token),
          Some(json!({
            "current_password": "a secure admin password", "new_password": ""
          })),
        )
        .await;
      assert_eq!(response.status, 422);
    }
    // Reaching verification now produces 500 before any post-verification limiter can run.
    // Exactly five 500s proves excess requests never enter verification, including sign-in's hashing queue.
    server
      .database()
      .execute("UPDATE users SET password_hash='malformed' WHERE username='alice'", [])
      .expect("verifier probe");
    assert_burst(&burst(&server, &signed, password_change, false).await, 500, 5);
    finish(server).await;
  }
}
