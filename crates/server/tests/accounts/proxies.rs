use serde_json::json;

use crate::support::{Response, TestServer};

/// A redemption of a code that does not exist: a failed join, counted for the client address.
async fn failed(server: &TestServer, headers: &[(&str, &str)]) -> Response {
  server
    .request_headers(
      "POST",
      "/auth/code",
      None,
      Some(json!({"code": "ZZZZ-ZZZZ", "device": "test"})),
      headers,
    )
    .await
}

/// The failed joins counted for `address`.
fn failures_from(server: &TestServer, address: &str) -> i64 {
  server
    .database()
    .query_row(
      "SELECT COALESCE(SUM(count),0) FROM failed_joins WHERE address=?1",
      [address],
      |row| row.get(0),
    )
    .expect("failure count")
}

#[tokio::test]
async fn s1a_forwarded_header_lines_cannot_spoof_the_appended_client() {
  let server = TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.0/8".parse().expect("proxy")]).await;
  for index in 0..10 {
    let forged = format!("203.0.113.{}", index + 1);
    let response = failed(
      &server,
      &[
        ("X-Forwarded-For", &forged),
        ("x-forwarded-for", "198.51.100.7, 127.0.0.2"),
        ("X-Forwarded-For", "127.0.0.3"),
      ],
    )
    .await;
    assert_eq!(response.status, 401);
    assert_eq!(failures_from(&server, "198.51.100.7"), index + 1);
  }
  let response = failed(
    &server,
    &[("X-Forwarded-For", "203.0.113.99"), ("X-Forwarded-For", "198.51.100.7")],
  )
  .await;
  assert_eq!(response.status, 429);
  assert_eq!(response.headers["retry-after"], "900");
  assert_eq!(
    failed(&server, &[("X-Forwarded-For", "198.51.100.8")]).await.status,
    401
  );
  server.finish().await;
}

#[tokio::test]
async fn s1a_forwarded_hops_support_cidr_mapped_ipv4_ports_and_brackets() {
  for proxy in ["127.0.0.0/8", "::ffff:127.0.0.1", "::ffff:127.0.0.0/104"] {
    let server = TestServer::start(|config| {
      config.trusted_proxies = vec![
        proxy.parse().expect("proxy"),
        "10.16.0.0/12".parse().expect("proxy CIDR"),
        "2001:db8:1::/48".parse().expect("IPv6 proxy CIDR"),
      ]
    })
    .await;
    for (hop, expected) in [
      ("198.51.100.7:4312", "198.51.100.7"),
      ("::ffff:198.51.100.7", "198.51.100.7"),
      ("[::ffff:198.51.100.7]:4312", "198.51.100.7"),
      ("[2001:db8:2::7]:4312", "2001:db8:2::7"),
      ("[2001:db8:2::8]", "2001:db8:2::8"),
    ] {
      let header = format!("203.0.113.9, {hop}, [2001:db8:1::99]:443, 10.25.1.9:123");
      let before = failures_from(&server, expected);
      assert_eq!(failed(&server, &[("X-Forwarded-For", &header)]).await.status, 401);
      assert_eq!(failures_from(&server, expected), before + 1);
    }
    server.finish().await;
  }
}

#[tokio::test]
async fn s1a_malformed_forwarding_in_any_line_falls_back_to_the_peer() {
  let server = TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.1".parse().expect("proxy")]).await;
  for malformed in [
    "bad address",
    "",
    "198.51.100.1:99999",
    "[2001:db8::1",
    "\u{80}",
    "198.51.100.1,,127.0.0.1",
  ] {
    let before = failures_from(&server, "127.0.0.1");
    assert_eq!(
      failed(
        &server,
        &[("X-Forwarded-For", malformed), ("X-Forwarded-For", "198.51.100.7")]
      )
      .await
      .status,
      401
    );
    assert_eq!(failures_from(&server, "127.0.0.1"), before + 1);
  }
  server.finish().await;
}

#[test]
fn s1a_invalid_proxy_networks_are_rejected() {
  for proxy in [
    "127.0.0.1/33",
    "::1/129",
    "::ffff:127.0.0.1/95",
    "127.0.0.1/-1",
    "127.0.0.1/24/8",
    "not an IP",
  ] {
    assert!(proxy.parse::<orchester_server::addresses::TrustedProxy>().is_err());
  }
}
