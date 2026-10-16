use std::{path::PathBuf, process::Command};

use orchester_api::ServerInfo;
use orchester_server::{
  Server,
  config::{Config, TlsMode},
  error::ServerError,
};

use crate::support::{TestServer, credential};

#[tokio::test]
async fn s1a_configured_tls_uses_a_real_trusted_certificate() {
  let server = TestServer::start(|config| {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    config.tls = TlsMode::Pem;
    config.tls_cert = Some(fixtures.join("server.pem"));
    config.tls_key = Some(fixtures.join("server-key.pem"));
  })
  .await;
  let response = server.tls_request().await;
  assert_eq!(response.status, 200);
  assert!(response.json::<ServerInfo>().needs_bootstrap);
  assert!(server.server.check_code().is_some());
  server.finish().await;
}

/// The code a host command printed to its own output: `<label> ABCD-EFGH`.
fn printed_code(output: &std::process::Output, label: &str) -> String {
  let stdout = String::from_utf8(output.stdout.clone()).expect("stdout");
  stdout
    .lines()
    .find_map(|line| line.strip_prefix(label))
    .map(|code| code.trim().to_owned())
    .unwrap_or_else(|| panic!("no {label} in the command's output"))
}

#[tokio::test]
async fn s1c_host_cli_prints_setup_and_account_codes_only_to_its_output_while_the_server_runs() {
  let server = TestServer::start(|_| {}).await;
  let binary = env!("CARGO_BIN_EXE_orchester-server");
  let command = |args: &[&str]| {
    Command::new(binary)
      .args(args)
      .env("ORCHESTER_SERVER_DATA_DIR", server.directory.path())
      .env("ORCHESTER_SERVER_TLS", "off")
      .output()
      .expect("host command")
  };
  let setup = command(&["setup-code"]);
  assert!(setup.status.success());
  let code = printed_code(&setup, "Setup code:");
  assert!(!String::from_utf8_lossy(&setup.stderr).contains(&code));
  let admin = server.join(&credential(&code), Some("cli_admin"), "laptop").await;
  assert!(admin.user.admin);
  let none = command(&["setup-code"]);
  assert!(none.status.success());
  assert!(String::from_utf8_lossy(&none.stdout).contains("has an admin already"));
  let account = command(&["account-code", "CLI_ADMIN"]);
  assert!(account.status.success());
  let code = printed_code(&account, "Account code:");
  assert!(!String::from_utf8_lossy(&account.stderr).contains(&code));
  let again = server.join(&credential(&code), None, "new laptop").await;
  assert_eq!(again.user.id, admin.user.id);
  let missing = command(&["account-code", "nobody"]);
  assert!(!missing.status.success());
  assert!(missing.stdout.is_empty());
  let removed = command(&["user", "create", "--admin", "someone"]);
  assert!(!removed.status.success());
  server.finish().await;
}

#[tokio::test]
async fn s1a_invalid_configuration_and_future_schema_are_refused() {
  let server = TestServer::start(|_| {}).await;
  for invalid in [
    Config {
      data_dir: server.directory.path().into(),
      tls_cert: Some("missing.pem".into()),
      ..Config::default()
    },
    Config {
      data_dir: server.directory.path().into(),
      tls: TlsMode::Pem,
      ..Config::default()
    },
    Config {
      data_dir: server.directory.path().into(),
      address: Some("https://orchester.example.com/path".into()),
      ..Config::default()
    },
  ] {
    assert!(matches!(
      Server::open(invalid, server.clock.clone()).await,
      Err(ServerError::Configuration(_))
    ));
  }
  let database = server.database();
  database
    .execute("UPDATE meta SET schema_version=999", [])
    .expect("future schema fixture");
  let config = Config {
    data_dir: server.directory.path().into(),
    tls: TlsMode::Off,
    ..Config::default()
  };
  assert!(matches!(
    Server::open(config, server.clock.clone()).await,
    Err(ServerError::Configuration(_))
  ));
  drop(database);
  server.finish().await;
}

/// Security review 1, minor 6: the private admin API listener on an address beyond loopback and private networks is
/// flagged, so the server warns at start.
#[test]
fn sa_an_admin_api_listener_beyond_private_networks_is_flagged() {
  let at = |address: &str| Config {
    admin_api_listen: Some(address.parse().expect("socket address")),
    ..Default::default()
  };
  for private in [
    "127.0.0.1:8741",
    "10.0.4.20:8741",
    "172.29.0.3:8741",
    "192.168.1.5:8741",
    "[::1]:8741",
    "[fd12::3]:8741",
    "[fe80::1]:8741",
    "[::ffff:10.0.0.1]:8741",
  ] {
    assert!(!at(private).admin_api_exposed(), "{private}");
  }
  for exposed in ["0.0.0.0:8741", "203.0.113.7:8741", "[::]:8741", "[2001:db8::1]:8741"] {
    assert!(at(exposed).admin_api_exposed(), "{exposed}");
  }
  let off = Config {
    admin_api_listen: None,
    ..Default::default()
  };
  assert!(!off.admin_api_exposed());
  // Security review 2, 4: the warning reads as one sentence, with no run of spaces from a wrapped line.
  assert!(!orchester_server::config::ADMIN_API_EXPOSED.contains("  "));
}
