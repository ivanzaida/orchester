use std::process::Command;

use orchester_api::{Joined, SettingsResponse, certificate::CertificatePin};
use orchester_server::{
  Server,
  config::{Config, TlsMode},
};
use serde_json::json;
use tokio_rustls::rustls::pki_types::{CertificateDer, pem::PemObject};

use crate::support::TestServer;

fn self_signed(config: &mut Config) {
  config.tls = TlsMode::SelfSigned;
  config.tls_names = vec!["orchester.internal".into(), "10.0.4.20".into()];
}

/// The check code of the certificate in the data directory, computed here from its file.
fn file_check_code(server: &TestServer) -> String {
  let path = server.directory.path().join("tls").join("cert.pem");
  let certificate = CertificateDer::from_pem_file(path).expect("generated certificate");
  CertificatePin::of(&certificate).check_code()
}

fn printed(output: &std::process::Output) -> String {
  assert!(output.status.success());
  let stdout = String::from_utf8(output.stdout.clone()).expect("stdout");
  let code = stdout
    .split_whitespace()
    .find(|word| word.len() == 19 && word.matches('-').count() == 3);
  code.expect("a check code in the output").to_owned()
}

#[tokio::test]
async fn s1c_the_server_generates_its_certificate_on_first_start_and_keeps_its_check_code() {
  let server = TestServer::start(self_signed).await;
  let code = server.server.check_code().expect("own TLS");
  assert_eq!(code.len(), 19);
  assert!(code.split('-').all(|group| group.len() == 4));
  assert_eq!(code, file_check_code(&server));
  let key = server.directory.path().join("tls").join("key.pem");
  assert!(key.exists());
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
      std::fs::metadata(&key).expect("key").permissions().mode() & 0o777,
      0o600
    );
  }
  let setup = server.server.host_setup_code().await.expect("setup").expect("no admin");
  let body = json!({"code": setup.expose(), "device": "laptop", "username": "alice", "display_name": "Alice"});
  let admin: Joined = server
    .tls_pinned_request("POST", "/auth/code", None, Some(body))
    .await
    .json();
  let settings: SettingsResponse = server
    .tls_pinned_request("GET", "/server/settings", Some(&admin.token), None)
    .await
    .json();
  assert_eq!(settings.check_code.as_deref(), Some(code.as_str()));
  let config = Config {
    data_dir: server.directory.path().into(),
    ..Config::default()
  };
  let reopened = Server::open(config, server.clock.clone()).await.expect("restart");
  assert_eq!(reopened.check_code(), Some(code));
  drop(reopened);
  server.finish().await;
}

#[tokio::test]
async fn s1c_tls_show_prints_the_check_code_and_regenerate_replaces_the_certificate() {
  let server = TestServer::start(self_signed).await;
  let code = server.server.check_code().expect("own TLS");
  let command = |args: &[&str]| {
    Command::new(env!("CARGO_BIN_EXE_orchester-server"))
      .args(args)
      .env("ORCHESTER_SERVER_DATA_DIR", server.directory.path())
      .env("ORCHESTER_SERVER_TLS", "self_signed")
      .output()
      .expect("tls command")
  };
  assert_eq!(printed(&command(&["tls", "show"])), code);
  let renewed = printed(&command(&["tls", "regenerate"]));
  assert_ne!(renewed, code);
  assert_eq!(file_check_code(&server), renewed);
  assert_eq!(printed(&command(&["tls", "show"])), renewed);
  let config = Config {
    data_dir: server.directory.path().into(),
    ..Config::default()
  };
  let restarted = Server::open(config, server.clock.clone()).await.expect("restart");
  assert_eq!(restarted.check_code(), Some(renewed));
  let off = Command::new(env!("CARGO_BIN_EXE_orchester-server"))
    .args(["tls", "show"])
    .env("ORCHESTER_SERVER_DATA_DIR", server.directory.path())
    .env("ORCHESTER_SERVER_TLS", "off")
    .output()
    .expect("tls show");
  assert!(String::from_utf8_lossy(&off.stdout).contains("plain HTTP"));
  drop(restarted);
  server.finish().await;
}
