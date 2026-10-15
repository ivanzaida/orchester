use orchester_api::{CloudLinkResponse, Credential, ErrorCode};
use serde_json::json;

use crate::support::{Response, TestServer, cloud::TestCloud, credential};

const MINUTE: i64 = 60 * 1000;

/// A server that takes the client address from `X-Forwarded-For` of a proxy on loopback.
async fn behind_a_proxy() -> TestServer {
  TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.1".parse().expect("proxy")]).await
}

/// A wrong code from `address`, or straight from loopback without one.
async fn guess(server: &TestServer, address: Option<&str>) -> Response {
  redeem_from(server, &credential("ZZZZ-ZZZZ"), address).await
}

async fn redeem_from(server: &TestServer, code: &Credential, address: Option<&str>) -> Response {
  let headers: Vec<(&str, &str)> = address
    .map(|address| ("X-Forwarded-For", address))
    .into_iter()
    .collect();
  let body = json!({"code": code.expose(), "device": "x", "username": "owner", "display_name": "Owner"});
  server
    .request_headers("POST", "/auth/code", None, Some(body), &headers)
    .await
}

/// 200 failures from 20 addresses, 10 each: the server-wide budget of an hour, spent.
async fn spend_the_server_budget(server: &TestServer) {
  for index in 0..200 {
    let address = format!("203.0.113.{}", index / 10 + 1);
    assert_eq!(guess(server, Some(&address)).await.status, 401, "failure {index}");
  }
}

#[tokio::test]
async fn s1c_the_server_wide_redemption_budget_slides_over_an_hour() {
  let server = behind_a_proxy().await;
  for index in 0..100 {
    let address = format!("198.51.100.{}", index / 10 + 1);
    assert_eq!(guess(&server, Some(&address)).await.status, 401);
  }
  server.clock.advance(30 * MINUTE);
  for index in 0..100 {
    let address = format!("203.0.113.{}", index / 10 + 1);
    assert_eq!(guess(&server, Some(&address)).await.status, 401);
  }
  let paused = guess(&server, Some("192.0.2.1")).await;
  assert_eq!(paused.status, 429);
  // The first 100 failures leave the hour 30 minutes from now; until then the budget stays spent.
  assert_eq!(paused.headers["retry-after"], "1800");
  server.clock.advance(15 * MINUTE);
  assert_eq!(guess(&server, Some("192.0.2.1")).await.status, 429);
  server.clock.advance(15 * MINUTE);
  assert_eq!(guess(&server, Some("192.0.2.1")).await.status, 401);
  server.finish().await;
}

#[tokio::test]
async fn s1c_a_spent_server_budget_still_lets_the_host_redeem_the_setup_code() {
  let server = behind_a_proxy().await;
  let setup = server
    .server
    .host_setup_code()
    .await
    .expect("setup code")
    .expect("no admin yet");
  spend_the_server_budget(&server).await;
  assert_eq!(redeem_from(&server, &setup, Some("192.0.2.1")).await.status, 429);
  // Straight from this computer, without a proxy's header, the setup code still works.
  assert_eq!(redeem_from(&server, &setup, None).await.status, 201);
  server.finish().await;
}

#[tokio::test]
async fn s1c_failed_joins_from_one_ipv6_48_count_together() {
  let server = behind_a_proxy().await;
  for index in 0..10 {
    let address = format!("2001:db8:7:{index:x}::1");
    assert_eq!(guess(&server, Some(&address)).await.status, 401);
  }
  assert_eq!(guess(&server, Some("2001:db8:7:ffff::1")).await.status, 429);
  assert_eq!(guess(&server, Some("2001:db8:8::1")).await.status, 401);
  server.finish().await;
}

#[tokio::test]
async fn s1c_failed_joins_are_counted_per_address_and_hour_not_audited_one_by_one() {
  let server = behind_a_proxy().await;
  for _ in 0..3 {
    assert_eq!(guess(&server, Some("198.51.100.7")).await.status, 401);
  }
  assert_eq!(guess(&server, Some("198.51.100.8")).await.status, 401);
  let database = server.database();
  let counts: Vec<(String, String, i64)> = database
    .prepare("SELECT kind,address,count FROM failed_joins ORDER BY address")
    .expect("counts")
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
    .expect("rows")
    .collect::<Result<_, _>>()
    .expect("count rows");
  assert_eq!(
    counts,
    [
      ("code".to_owned(), "198.51.100.7".to_owned(), 3),
      ("code".to_owned(), "198.51.100.8".to_owned(), 1)
    ]
  );
  let audited: i64 = database
    .query_row(
      "SELECT COUNT(*) FROM audit_events WHERE action='join_failed'",
      [],
      |row| row.get(0),
    )
    .expect("audit");
  assert_eq!(audited, 0);
  server.finish().await;
}

/// A server behind a proxy, linked to `cloud` by its first admin.
async fn linked_behind_a_proxy(cloud: &TestCloud) -> TestServer {
  let server = behind_a_proxy().await;
  let admin = server.bootstrap().await;
  let linked: CloudLinkResponse = server
    .request(
      "PATCH",
      "/server/cloud",
      Some(&admin.token),
      Some(json!({"link": {"root_key": cloud.root.public(), "key_set": cloud.key_set(server.seconds())}})),
    )
    .await
    .json();
  assert!(linked.cloud.accepting);
  server
}

async fn bad_cloud_join(server: &TestServer, address: &str) -> Response {
  let body = json!({"identity_token": "not.a.token", "device": "x"});
  server
    .request_headers("POST", "/auth/cloud", None, Some(body), &[("X-Forwarded-For", address)])
    .await
}

#[tokio::test]
async fn s1c_failed_cloud_joins_share_the_address_limit_and_have_a_server_budget_of_their_own() {
  let cloud = TestCloud::generate();
  let server = linked_behind_a_proxy(&cloud).await;
  for index in 0..10 {
    let address = format!("2001:db8:5:{index:x}::1");
    let refused = bad_cloud_join(&server, &address).await;
    assert_eq!(refused.status, 401);
    assert_eq!(
      refused.json::<orchester_api::ApiError>().error,
      ErrorCode::InvalidIdentity
    );
  }
  assert_eq!(guess(&server, Some("2001:db8:5:ffff::1")).await.status, 429);
  for index in 0..190 {
    let address = format!("203.0.113.{}", index / 10 + 1);
    assert_eq!(bad_cloud_join(&server, &address).await.status, 401, "failure {index}");
  }
  assert_eq!(bad_cloud_join(&server, "192.0.2.1").await.status, 429);
  // Codes have a budget of their own: failed cloud joins do not pause them.
  assert_eq!(guess(&server, Some("192.0.2.1")).await.status, 401);
  server.finish().await;
}
