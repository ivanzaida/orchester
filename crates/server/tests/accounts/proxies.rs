use serde_json::json;

use crate::support::{Response, TestServer};

async fn failed(server: &TestServer, name: &str, headers: &[(&str, &str)]) -> Response {
  server
    .request_headers(
      "POST",
      "/auth/sessions",
      None,
      Some(json!({"username": name, "password": "wrong test password", "device": "test"})),
      headers,
    )
    .await
}

fn audited_address(server: &TestServer, name: &str) -> String {
  server
    .database()
    .query_row(
      "SELECT address FROM audit_events WHERE action='sign_in_failed' AND target=?1",
      [name],
      |row| row.get(0),
    )
    .expect("failure address")
}

#[tokio::test]
async fn s1a_forwarded_header_lines_cannot_spoof_the_appended_client() {
  let server = TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.0/8".parse().expect("proxy")]).await;
  for index in 0..20 {
    let forged = format!("203.0.113.{}", index + 1);
    let name = format!("missing{index}");
    let response = failed(
      &server,
      &name,
      &[
        ("X-Forwarded-For", &forged),
        ("x-forwarded-for", "198.51.100.7, 127.0.0.2"),
        ("X-Forwarded-For", "127.0.0.3"),
      ],
    )
    .await;
    assert_eq!(response.status, 401);
    assert_eq!(audited_address(&server, &name), "198.51.100.7");
  }
  let response = failed(
    &server,
    "blocked",
    &[("X-Forwarded-For", "203.0.113.99"), ("X-Forwarded-For", "198.51.100.7")],
  )
  .await;
  assert_eq!(response.status, 429);
  assert_eq!(response.headers["retry-after"], "900");
  assert_eq!(
    failed(&server, "other", &[("X-Forwarded-For", "198.51.100.8")])
      .await
      .status,
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
    for (index, (hop, expected)) in [
      ("198.51.100.7:4312", "198.51.100.7"),
      ("::ffff:198.51.100.7", "198.51.100.7"),
      ("[::ffff:198.51.100.7]:4312", "198.51.100.7"),
      ("[2001:db8:2::7]:4312", "2001:db8:2::7"),
      ("[2001:db8:2::8]", "2001:db8:2::8"),
    ]
    .into_iter()
    .enumerate()
    {
      let name = format!("port{index}");
      let header = format!("203.0.113.9, {hop}, [2001:db8:1::99]:443, 10.25.1.9:123");
      assert_eq!(
        failed(&server, &name, &[("X-Forwarded-For", &header)]).await.status,
        401
      );
      assert_eq!(audited_address(&server, &name), expected);
    }
    server.finish().await;
  }
}

#[tokio::test]
async fn s1a_malformed_forwarding_in_any_line_falls_back_to_the_peer() {
  let server = TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.1".parse().expect("proxy")]).await;
  for (index, malformed) in [
    "bad address",
    "",
    "198.51.100.1:99999",
    "[2001:db8::1",
    "\u{80}",
    "198.51.100.1,,127.0.0.1",
  ]
  .into_iter()
  .enumerate()
  {
    let name = format!("malformed{index}");
    assert_eq!(
      failed(
        &server,
        &name,
        &[("X-Forwarded-For", malformed), ("X-Forwarded-For", "198.51.100.7")]
      )
      .await
      .status,
      401
    );
    assert_eq!(audited_address(&server, &name), "127.0.0.1");
  }
  server.finish().await;
}

#[tokio::test]
async fn s1a_ipv6_address_rate_limit_groups_a_64_and_keeps_other_networks_separate() {
  let server = TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.1".parse().expect("proxy")]).await;
  for index in 0..20 {
    let address = format!("2001:db8:7:8::{index:x}");
    assert_eq!(
      failed(&server, &format!("v6_{index}"), &[("X-Forwarded-For", &address)])
        .await
        .status,
      401
    );
  }
  assert_eq!(
    failed(&server, "same_network", &[("X-Forwarded-For", "2001:db8:7:8:ffff::1")])
      .await
      .status,
    429
  );
  assert_eq!(
    failed(&server, "other_network", &[("X-Forwarded-For", "2001:db8:7:9::1")])
      .await
      .status,
    401
  );
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
