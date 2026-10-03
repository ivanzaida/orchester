use std::process::Command;

use orchester_server::config::Config;

use crate::support::{TestServer, credential};

#[tokio::test]
async fn s1a_toml_configuration_and_environment_override_for_host_commands() {
  let server = TestServer::start(|_| {}).await;
  let configuration = server.directory.path().join("server.toml");
  let directory = server.directory.path().to_string_lossy().replace('\\', "/");
  std::fs::write(&configuration, format!("listen = \"127.0.0.1:0\"\nname = \"Test server\"\ndata_dir = \"{directory}\"\ntrusted_proxies = [\"127.0.0.1\", \"::1\"]\n")).expect("configuration");
  let config = Config::load(Some(&configuration)).expect("load TOML");
  assert_eq!(config.name, "Test server");
  assert_eq!(config.trusted_proxies.len(), 2);
  let output = Command::new(env!("CARGO_BIN_EXE_orchester-server"))
    .arg("--config")
    .arg(&configuration)
    .args(["user", "create", "--admin", "toml_admin"])
    .env_remove("ORCHESTER_SERVER_DATA_DIR")
    .output()
    .expect("configured command");
  assert!(output.status.success());
  let password = credential(std::str::from_utf8(&output.stdout).expect("password output").trim());
  assert!(server.login("toml_admin", &password).await.password_change_required);
  let overridden = server.directory.path().join("override");
  let output = Command::new(env!("CARGO_BIN_EXE_orchester-server"))
    .arg("--config")
    .arg(&configuration)
    .args(["user", "create", "--admin", "env_admin"])
    .env("ORCHESTER_SERVER_DATA_DIR", &overridden)
    .output()
    .expect("environment command");
  assert!(output.status.success());
  assert!(overridden.join("orchester-server.db").exists());
  let found: bool = server
    .database()
    .query_row(
      "SELECT EXISTS(SELECT 1 FROM users WHERE username='env_admin')",
      [],
      |row| row.get(0),
    )
    .expect("database");
  assert!(!found);
  server.finish().await;
}
