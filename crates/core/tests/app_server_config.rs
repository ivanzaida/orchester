//! What a Codex App Server `config/read` result with layers says about the repository's own `.codex`
//! folder. Run with `cargo test -p orchester-core --test app_server_config config_read`.

use std::collections::BTreeSet;

use orchester_core::{
  connection::app_server_config::{ConfigReadError, refuse_taken_names, repository_settings},
  orchester_tools::SERVER,
};
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

/// A server of the user's own configuration with the name Codex gets a chosen one by would merge with it: one the
/// configuration did not have when the step's servers were named apart from the user's (`CodexMcp::named_apart`).
#[test]
fn config_read_user_server_named_like_a_chosen_one_refuses_the_step() {
  let selected = BTreeSet::from(["docs".to_owned()]);
  let layers = |user: serde_json::Value| {
    json!({"layers": [
      {"name":{"type":"sessionFlags"},"config":{"mcp_servers":{"docs":{"command":"npx"}}}},
      {"name":{"type":"user","file":"config.toml"},"config":{"mcp_servers": user}},
      {"name":{"type":"project","dotCodexFolder":"/repo/.codex"},"config":{"mcp_servers":{"docs":{}}},
       "disabledReason":"untrusted"}
    ]})
  };
  assert_eq!(refuse_taken_names(&layers(json!({"other": {}})), &selected), Ok(()));
  assert_eq!(
    refuse_taken_names(&layers(json!({"docs": {"env": {"TOKEN": "user"}}})), &selected),
    Err(ConfigReadError::ServerNameTaken("docs".to_owned()))
  );
}

/// The app's own MCP server, registered in the user's Codex as `orchester`, does not take the name of the runs'
/// relay `Orchester`: Codex tells the two apart, so the step starts and turns the user's one off.
#[test]
fn config_read_app_mcp_user_orchester_server_does_not_take_the_relays_name() {
  let selected = BTreeSet::from([SERVER.to_owned()]);
  let layers = |name: &str| {
    json!({"layers": [
      {"name":{"type":"sessionFlags"},"config":{"mcp_servers":{SERVER:{"command":"orchester-runner"}}}},
      {"name":{"type":"user","file":"config.toml"},"config":{"mcp_servers":{name:{
        "url":"http://127.0.0.1:47821/mcp","bearer_token_env_var":"ORCHESTER_APP_MCP_TOKEN"
      }}}}
    ]})
  };
  assert_eq!(refuse_taken_names(&layers("orchester"), &selected), Ok(()));
  assert_eq!(
    refuse_taken_names(&layers(SERVER), &selected),
    Err(ConfigReadError::ServerNameTaken(SERVER.to_owned()))
  );
}
