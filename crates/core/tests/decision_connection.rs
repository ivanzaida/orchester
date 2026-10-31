//! A decision model's connection (`docs/design/decision-reviewer.md`): its server's base URL rules, whether that
//! server is on this computer, and that no key value shows in it. No agent profile can choose one: it has its own id
//! type, which a profile's connection (a `ConnectionId`) cannot hold.

use orchester_core::{
  connection::{DecisionConnection, DecisionServerUrl, DecisionServerUrlError, is_this_computer},
  id::{DecisionConnectionId, SecretId},
  name::Name,
  resource::ModelId,
};

fn url(value: &str) -> DecisionServerUrl {
  DecisionServerUrl::new(value).expect("valid base URL")
}

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("valid model id")
}

fn connection(base_url: &str, key: Option<SecretId>) -> DecisionConnection {
  DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new("System One").expect("valid name"),
    url(base_url),
    key,
  )
}

#[test]
fn decision_connection_base_url_takes_http_or_https_on_any_host_without_a_trailing_slash() {
  for (given, kept) in [
    ("http://127.0.0.1:8080", "http://127.0.0.1:8080"),
    ("  https://decisions.example.com/  ", "https://decisions.example.com"),
    ("HTTP://192.168.1.20:9000", "HTTP://192.168.1.20:9000"),
    ("http://[::1]:8080/", "http://[::1]:8080"),
    ("https://example.com/api/v2//", "https://example.com/api/v2"),
    (
      "https://example.com/systemone-proxy",
      "https://example.com/systemone-proxy",
    ),
  ] {
    assert_eq!(url(given).as_str(), kept, "{given}");
  }
}

#[test]
fn decision_connection_base_url_refuses_what_is_no_server_base() {
  let long = format!("https://example.com/{}", "a".repeat(2048));
  for (given, error) in [
    ("", DecisionServerUrlError::Blank),
    ("   ", DecisionServerUrlError::Blank),
    ("ftp://example.com", DecisionServerUrlError::Scheme),
    ("example.com", DecisionServerUrlError::Scheme),
    ("http://", DecisionServerUrlError::NoHost),
    ("http://:8080", DecisionServerUrlError::NoHost),
    ("http://user:pass@example.com", DecisionServerUrlError::Credentials),
    ("http://example.com?model=a", DecisionServerUrlError::Query),
    ("http://example.com/#part", DecisionServerUrlError::Query),
    ("http://exa mple.com", DecisionServerUrlError::Whitespace),
    ("http://example.com\\path", DecisionServerUrlError::Backslash),
    ("http://example.com:port", DecisionServerUrlError::InvalidHost),
    ("http://[::1", DecisionServerUrlError::InvalidHost),
    ("http://[fe80::1%25eth0]", DecisionServerUrlError::InvalidHost),
    ("http://ex%41mple.com", DecisionServerUrlError::InvalidHost),
    (long.as_str(), DecisionServerUrlError::TooLong),
    ("http://127.0.0.1:8080/systemone", DecisionServerUrlError::ApiPath),
    ("http://127.0.0.1:8080/SystemOne/", DecisionServerUrlError::ApiPath),
    (
      "https://example.com/proxy/systemone/v1",
      DecisionServerUrlError::ApiPath,
    ),
  ] {
    assert_eq!(DecisionServerUrl::new(given), Err(error), "{given}");
  }
}

#[test]
fn decision_connection_base_url_round_trips_through_its_stored_text() {
  let stored = serde_json::to_string(&url("http://localhost:7000/")).expect("serializes");
  assert_eq!(stored, "\"http://localhost:7000\"");
  let read: DecisionServerUrl = serde_json::from_str(&stored).expect("deserializes");
  assert_eq!(read.as_str(), "http://localhost:7000");
  assert!(serde_json::from_str::<DecisionServerUrl>("\"http://localhost:7000/systemone\"").is_err());
}

#[test]
fn decision_connection_on_this_computer_is_loopback_by_name_only() {
  for host in ["127.0.0.1", "::1", "[::1]", "localhost", "LocalHost", "LOCALHOST"] {
    assert!(is_this_computer(host), "{host}");
  }
  for host in [
    "0.0.0.0",
    "127.0.0.2",
    "127.1",
    "[0:0:0:0:0:0:0:1]",
    "localhost.",
    "localhost.example.com",
    "mylocalhost",
    "192.168.1.20",
    "10.0.0.5",
    "example.com",
    "",
  ] {
    assert!(!is_this_computer(host), "{host}");
  }
}

#[test]
fn decision_connection_on_this_computer_reads_the_host_without_its_port_or_path() {
  for local in [
    "http://127.0.0.1",
    "http://127.0.0.1:8080/api",
    "https://localhost:443",
    "http://LOCALHOST:7000",
    "http://[::1]:9000",
  ] {
    assert!(url(local).is_on_this_computer(), "{local}");
    assert!(connection(local, None).is_on_this_computer(), "{local}");
  }
  for other in [
    "http://0.0.0.0:8080",
    "http://192.168.1.20:8080",
    "https://decisions.example.com",
    "http://localhost.example.com:8080",
    "http://[::2]:8080",
  ] {
    assert!(!url(other).is_on_this_computer(), "{other}");
  }
  assert_eq!(url("http://[::1]:9000/api").host(), "[::1]");
  assert_eq!(url("https://Example.com:8443").host(), "Example.com");
}

#[test]
fn decision_connection_keeps_its_listed_models_once_each_until_the_server_or_key_changes() {
  let key = SecretId::generate();
  let mut decision = connection("http://127.0.0.1:8080", Some(key)).with_models(vec![
    model("judge-small"),
    model("judge-large"),
    model("judge-small"),
  ]);
  assert_eq!(decision.models(), &[model("judge-small"), model("judge-large")]);
  assert_eq!(decision.key(), Some(key));

  decision.set_server(url("http://127.0.0.1:8080/"), Some(key));
  assert_eq!(decision.models().len(), 2, "the same server and key keep the models");

  decision.set_server(url("http://127.0.0.1:8080"), None);
  assert!(decision.models().is_empty(), "another key drops them");
  assert_eq!(decision.key(), None);

  decision.record_models(vec![model("judge-small")]);
  decision.set_server(url("http://127.0.0.1:9090"), None);
  assert!(decision.models().is_empty(), "another server drops them");
}

#[test]
fn decision_connection_shows_no_key_value_in_debug_or_errors() {
  const PASSWORD: &str = "sk-decision-fake-password";
  let key = SecretId::generate();
  let decision = connection("https://decisions.example.com", Some(key));
  let debug = format!("{decision:?}");
  assert!(debug.contains(&key.to_string()), "the key is named by its secret only");

  let error = DecisionServerUrl::new(&format!("https://reviewer:{PASSWORD}@decisions.example.com"))
    .expect_err("credentials in the URL are refused");
  assert_eq!(error, DecisionServerUrlError::Credentials);
  let shown = format!("{error:?} {error}");
  assert!(!shown.contains(PASSWORD), "{shown}");
  assert!(!shown.contains("reviewer"), "{shown}");
}
