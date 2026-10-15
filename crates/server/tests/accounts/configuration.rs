use std::process::Command;

use orchester_server::config::{Config, TlsMode};

use crate::support::TestServer;

#[tokio::test]
async fn s1a_toml_configuration_and_environment_override_for_host_commands() {
  let server = TestServer::start(|_| {}).await;
  let configuration = server.directory.path().join("server.toml");
  let directory = server.directory.path().to_string_lossy().replace('\\', "/");
  std::fs::write(
    &configuration,
    format!(
      "listen = \"127.0.0.1:0\"\nname = \"Test server\"\ndata_dir = \"{directory}\"\ntls = \"off\"\naddress = \"https://orchester.example.com\"\ntrusted_proxies = [\"127.0.0.1\", \"::1\"]\n"
    ),
  )
  .expect("configuration");
  let config = Config::load(Some(&configuration)).expect("load TOML");
  assert_eq!(config.name, "Test server");
  assert_eq!(config.tls, TlsMode::Off);
  assert_eq!(config.address.as_deref(), Some("https://orchester.example.com"));
  assert_eq!(config.trusted_proxies.len(), 2);
  let output = Command::new(env!("CARGO_BIN_EXE_orchester-server"))
    .arg("--config")
    .arg(&configuration)
    .arg("setup-code")
    .env_remove("ORCHESTER_SERVER_DATA_DIR")
    .env_remove("ORCHESTER_SERVER_TLS")
    .output()
    .expect("configured command");
  assert!(output.status.success());
  assert!(String::from_utf8_lossy(&output.stdout).starts_with("Setup code: "));
  assert!(
    !server.directory.path().join("tls").exists(),
    "tls = \"off\" generates no certificate"
  );
  let overridden = server.directory.path().join("override");
  let output = Command::new(env!("CARGO_BIN_EXE_orchester-server"))
    .arg("--config")
    .arg(&configuration)
    .arg("setup-code")
    .env("ORCHESTER_SERVER_DATA_DIR", &overridden)
    .env("ORCHESTER_SERVER_TLS", "self_signed")
    .output()
    .expect("environment command");
  assert!(output.status.success());
  assert!(overridden.join("orchester-server.db").exists());
  assert!(overridden.join("tls").join("cert.pem").exists());
  let codes: i64 = server
    .database()
    .query_row("SELECT COUNT(*) FROM codes WHERE kind='setup'", [], |row| row.get(0))
    .expect("database");
  assert_eq!(codes, 1);
  server.finish().await;
}
