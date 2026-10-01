//! What a Codex App Server `config/read` result with layers says about the repository's own `.codex`
//! folder. Run with `cargo test -p orchester-core --test app_server_config config_read`.

use orchester_core::connection::app_server_config::{ConfigReadError, repository_settings};
use serde_json::json;

fn user_layers() -> Vec<serde_json::Value> {
  vec![
    json!({"name":{"type":"user","file":"C:\\Users\\me\\.codex\\config.toml"},"config":{}}),
    json!({"name":{"type":"sessionFlags"},"config":{}}),
  ]
}

#[test]
fn config_read_without_a_repository_folder_notes_nothing() {
  assert_eq!(repository_settings(&json!({"layers": user_layers()})), Ok(None));
}

#[test]
fn config_read_repository_folder_left_out_is_noted() {
  let mut layers = user_layers();
  layers.push(json!({
    "name":{"type":"project","dotCodexFolder":"h:\\repo\\.orchester\\worktrees\\run\\.codex"},
    "config":{"mcp_servers":{"repository":{}}},
    "disabledReason":"h:\\repo\\.orchester\\worktrees\\run is marked as untrusted in the effective configuration."
  }));
  assert_eq!(
    repository_settings(&json!({"layers": layers})),
    Ok(Some("h:\\repo\\.orchester\\worktrees\\run\\.codex".to_owned()))
  );
}

#[test]
fn config_read_repository_folder_loaded_refuses_the_step() {
  let mut layers = user_layers();
  layers.push(json!({
    "name":{"type":"project","dotCodexFolder":"/repo/.codex"},
    "config":{},
    "disabledReason":null
  }));
  assert_eq!(
    repository_settings(&json!({"layers": layers})),
    Err(ConfigReadError::RepositorySettingsLoaded("/repo/.codex".to_owned()))
  );
}

#[test]
fn config_read_without_layers_cannot_be_checked() {
  assert_eq!(
    repository_settings(&json!({"config": {}})),
    Err(ConfigReadError::MissingLayers)
  );
}
