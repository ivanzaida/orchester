//! `proxy_certificate = "auto"` (`docs/server-hosting.md`, "The server admin"): behind a proxy that holds the
//! certificate, the server reads the one its own public address presents, checked by the trust store, chain and name,
//! and names it in its key proofs; it follows a renewal, still names the certificate before one for a while to an app
//! that says which one its connection presented, and names none when the read fails. And the configuration around it:
//! `auto` only with TLS off and an https address, and an empty value is the same as none.

use std::process::Command;

use orchester_api::{
  KeyProofResponse, PRESENTED_CERTIFICATE,
  client::{Client, PlainHttp, ServerAddress},
  identity::{key_proof_nonce, read_statement, verify_key_proof},
  statements::{KEY_PROOF_TYPE, KeyProof},
};
use orchester_server::{
  config::{Config, ProxyCertificate, TlsMode},
  error::ServerError,
};
use serde_json::json;
use tokio::net::TcpListener;

use crate::support::{
  TestServer,
  tls_proxy::{Authority, Leaf, TlsProxy, trusted},
};

const MINUTE_MS: i64 = 60 * 1000;

/// A server with `proxy_certificate = "auto"` whose address is the proxy's, which presents `leaf`.
async fn behind_proxy(leaf: &Leaf) -> (TestServer, TlsProxy, String) {
  let listener = TcpListener::bind("127.0.0.1:0").await.expect("the proxy's port");
  let address = format!("https://localhost:{}", listener.local_addr().expect("address").port());
  let configured = address.clone();
  let server = TestServer::start(move |config| {
    config.address = Some(configured);
    config.proxy_certificate = Some("auto".into());
  })
  .await;
  let proxy = TlsProxy::start(listener, leaf, server.address());
  (server, proxy, address)
}

/// What the server's key proof names, asked directly, with the certificate the app's connection presented if any.
async fn named(server: &TestServer, presented: Option<&Leaf>) -> KeyProof {
  let nonce = key_proof_nonce().expect("a nonce");
  let hint = presented.map(Leaf::sha256);
  let headers: Vec<(&str, &str)> = hint.iter().map(|hint| (PRESENTED_CERTIFICATE, hint.as_str())).collect();
  let body = Some(json!({"nonce": nonce}));
  let answer = server
    .request_headers("POST", "/server/key-proof", None, body, &headers)
    .await;
  assert_eq!(answer.status, 200);
  read_statement::<KeyProof>(&answer.json::<KeyProofResponse>().proof, KEY_PROOF_TYPE).expect("a key proof")
}

#[tokio::test]
async fn proxy_certificate_auto_names_what_the_address_presents_and_the_app_accepts_it() {
  let leaf = trusted().leaf("localhost");
  let (server, _proxy, address) = behind_proxy(&leaf).await;
  let proof = named(&server, None).await;
  assert_eq!(proof.certificate, Some(leaf.sha256()));
  assert_eq!(proof.previous_certificate, None);
  let malformed = server
    .request_headers(
      "POST",
      "/server/key-proof",
      None,
      Some(json!({"nonce": key_proof_nonce().expect("a nonce")})),
      &[(PRESENTED_CERTIFICATE, "not a fingerprint")],
    )
    .await;
  assert_eq!(malformed.status, 422, "a header that names no certificate");
  // An app reaching the server through the proxy checks the proof against its own connection's certificate.
  let id = server.audience().await.parse().expect("the server's id");
  let client = Client::new(ServerAddress::parse(&address).expect("address"), PlainHttp::NotAllowed).expect("client");
  client.server_info().await.expect("through the proxy");
  let nonce = key_proof_nonce().expect("a nonce");
  let answer = client.key_proof(&nonce).await.expect("a key proof");
  let certificate = client.certificate().expect("a verified connection").to_hex();
  assert_eq!(certificate, leaf.sha256());
  verify_key_proof(&answer, id, (&nonce, &certificate)).expect("the proof names this connection's certificate");
  server.finish().await;
}

#[tokio::test]
async fn proxy_certificate_auto_follows_a_renewal_and_keeps_the_one_before_for_a_while() {
  let authority = trusted();
  let (first, renewed) = (authority.leaf("localhost"), authority.leaf("localhost"));
  let (server, proxy, _) = behind_proxy(&first).await;
  assert_eq!(named(&server, None).await.certificate, Some(first.sha256()));
  proxy.present(&renewed);
  // Read a moment ago: an app that says nothing gets the same one until it is read again.
  assert_eq!(named(&server, None).await.certificate, Some(first.sha256()));
  // An app whose connection presented one the server has not seen makes it read again, at most twice a minute.
  server.clock.advance(MINUTE_MS);
  let proof = named(&server, Some(&renewed)).await;
  assert_eq!(proof.certificate, Some(renewed.sha256()));
  assert_eq!(proof.previous_certificate, Some(first.sha256()));
  // A connection made before the switch still checks out; an app that names no certificate never gets that field.
  assert_eq!(
    named(&server, Some(&first)).await.previous_certificate,
    Some(first.sha256())
  );
  let silent = named(&server, None).await;
  assert_eq!(silent.certificate, Some(renewed.sha256()));
  assert_eq!(silent.previous_certificate, None);
  // An hour after the renewal the one before is no longer named.
  server.clock.advance(61 * MINUTE_MS);
  let later = named(&server, Some(&first)).await;
  assert_eq!(later.certificate, Some(renewed.sha256()));
  assert_eq!(later.previous_certificate, None);
  // A read older than ten minutes is made again before a proof, whatever the app says.
  let third = authority.leaf("localhost");
  proxy.present(&third);
  server.clock.advance(11 * MINUTE_MS);
  assert_eq!(named(&server, None).await.certificate, Some(third.sha256()));
  server.finish().await;
}

#[tokio::test]
async fn proxy_certificate_auto_names_none_when_the_read_fails() {
  // Nothing listens at the address.
  let closed = TcpListener::bind("127.0.0.1:0").await.expect("a port");
  let port = closed.local_addr().expect("address").port();
  drop(closed);
  let unreachable = TestServer::start(|config| {
    config.address = Some(format!("https://localhost:{port}"));
    config.proxy_certificate = Some("auto".into());
  })
  .await;
  assert_eq!(named(&unreachable, None).await.certificate, None);
  unreachable.finish().await;
  // A certificate from an authority nobody trusts, or for another name: a hijacked or broken route.
  let authority = trusted();
  let stranger = Authority::generate("Not trusted").leaf("localhost");
  for presented in [stranger, authority.leaf("elsewhere.test")] {
    let (server, _proxy, _) = behind_proxy(&presented).await;
    assert_eq!(named(&server, Some(&presented)).await.certificate, None);
    server.finish().await;
  }
  // A read that fails after one succeeded forgets what the route showed before.
  let leaf = authority.leaf("localhost");
  let (server, proxy, _) = behind_proxy(&leaf).await;
  assert_eq!(named(&server, None).await.certificate, Some(leaf.sha256()));
  proxy.present(&Authority::generate("Hijacker").leaf("localhost"));
  server.clock.advance(11 * MINUTE_MS);
  let proof = named(&server, Some(&leaf)).await;
  assert_eq!((proof.certificate, proof.previous_certificate), (None, None));
  server.finish().await;
}

#[test]
fn proxy_certificate_auto_only_with_tls_off_and_an_https_address() {
  let config = |tls, address: Option<&str>, proxy: &str| Config {
    tls,
    address: address.map(str::to_owned),
    proxy_certificate: Some(proxy.to_owned()),
    ..Config::default()
  };
  let https = Some("https://orchester.example.com");
  let refused = [
    config(TlsMode::SelfSigned, https, "auto"),
    config(TlsMode::Off, None, "auto"),
    config(TlsMode::Off, Some("http://orchester.example.com"), "auto"),
    config(TlsMode::Off, https, "not a fingerprint"),
  ];
  for refused in refused {
    assert!(
      matches!(refused.validate(), Err(ServerError::Configuration(_))),
      "{:?}",
      refused.proxy_certificate
    );
  }
  let auto = config(TlsMode::Off, https, "AUTO");
  auto.validate().expect("auto behind a proxy");
  assert_eq!(auto.proxy_certificate().expect("valid"), Some(ProxyCertificate::Auto));
  let fixed = config(TlsMode::Off, None, &("AB:".repeat(31) + "AB"));
  assert!(matches!(
    fixed.proxy_certificate().expect("valid"),
    Some(ProxyCertificate::Fixed(pin)) if pin.to_hex() == "ab".repeat(32)
  ));
  // Empty is the same as unset, with any TLS mode.
  for empty in ["", "  "] {
    let unset = config(TlsMode::SelfSigned, None, empty);
    unset.validate().expect("empty means unset");
    assert_eq!(unset.proxy_certificate().expect("valid"), None);
  }
}

#[tokio::test]
async fn proxy_certificate_an_empty_environment_value_is_unset() {
  let server = TestServer::start(|_| {}).await;
  let run = |value: &str| {
    Command::new(env!("CARGO_BIN_EXE_orchester-server"))
      .arg("setup-code")
      .env("ORCHESTER_SERVER_DATA_DIR", server.directory.path().join("environment"))
      .env("ORCHESTER_SERVER_TLS", "self_signed")
      .env("ORCHESTER_SERVER_PROXY_CERTIFICATE", value)
      .output()
      .expect("the host command")
  };
  assert!(run("").status.success(), "an empty value is no proxy certificate");
  assert!(!run("auto").status.success(), "auto needs tls = \"off\"");
  server.finish().await;
}
