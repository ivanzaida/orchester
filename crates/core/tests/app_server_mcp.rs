use std::collections::BTreeSet;

use orchester_core::connection::app_server_mcp::{
  McpInventoryError, UnavailableServer, refuse_user_hooks, servers_to_turn_off, unavailable_selected,
  verify_only_chosen_run, verify_selected_servers,
};
use serde_json::json;

#[test]
fn selected_server_is_checked_and_extra_global_servers_remain_available() {
  let selected = BTreeSet::from(["project-pencil".to_owned()]);
  let inventory = json!({"data":[
    {"name":"global-docs","runtimeStatus":"connected","toolsError":null},
    {"name":"project-pencil","runtimeStatus":"connected","toolsError":null},
    {"name":"global-other","runtimeStatus":"failed","toolsError":"not needed"}
  ],"nextCursor":null});
  assert_eq!(verify_selected_servers(&inventory, &selected), Ok(()));
  let missing = json!({"data":[{"name":"global-docs","runtimeStatus":"connected"}],"nextCursor":null});
  assert_eq!(
    verify_selected_servers(&missing, &selected),
    Err(McpInventoryError::MissingSelected)
  );
}

#[test]
fn selected_server_must_be_connected() {
  let selected = BTreeSet::from(["project-pencil".to_owned()]);
  for server in [
    json!({"name":"project-pencil","runtimeStatus":"disabled","toolsError":null}),
    json!({"name":"project-pencil","runtimeStatus":"authenticationRequired","toolsError":null}),
    json!({"name":"project-pencil","runtimeStatus":"notStarted","toolsError":null}),
    json!({"name":"project-pencil","runtimeStatus":"starting","toolsError":null}),
    json!({"name":"project-pencil","runtimeStatus":null,"toolsError":null}),
    json!({"name":"project-pencil","runtimeStatus":"connected","toolsError":"private diagnostic"}),
  ] {
    let inventory = json!({"data":[server],"nextCursor":null});
    assert_eq!(
      verify_selected_servers(&inventory, &selected),
      Err(McpInventoryError::UnavailableSelected)
    );
  }
}

/// A step that does not ask before MCP actions works without the selected servers that are not
/// connected, each named with its state, never Codex's own diagnostic.
#[test]
fn selected_servers_that_are_not_connected_are_named_for_the_activity() {
  let selected = BTreeSet::from(["docs".to_owned(), "missing".to_owned(), "pencil".to_owned()]);
  let inventory = json!({"data":[
    {"name":"docs","runtimeStatus":"failed","toolsError":"private diagnostic"},
    {"name":"pencil","runtimeStatus":"connected","toolsError":null},
    {"name":"global-other","runtimeStatus":"failed","toolsError":null}
  ],"nextCursor":null});
  assert_eq!(
    unavailable_selected(&inventory, &selected),
    Ok(vec![
      UnavailableServer {
        name: "missing".to_owned(),
        state: None
      },
      UnavailableServer {
        name: "docs".to_owned(),
        state: Some("failed".to_owned())
      },
    ])
  );
  assert_eq!(
    unavailable_selected(&json!({"data":[],"nextCursor":"more"}), &selected),
    Err(McpInventoryError::TooLarge)
  );
}

/// The user's own servers and a project's are turned off by name; the step's own are not.
#[test]
fn configured_servers_the_step_does_not_choose_are_turned_off() {
  let selected = BTreeSet::from(["docs".to_owned()]);
  let config = json!({"config": {"mcp_servers": {
    "docs": {"enabled": true}, "ida-pro-mcp": {"enabled": true}, "pencil": {"enabled": true}
  }}});
  assert_eq!(
    servers_to_turn_off(&config, &selected),
    Ok(vec!["ida-pro-mcp".to_owned(), "pencil".to_owned()])
  );
  assert_eq!(
    servers_to_turn_off(&json!({"config": {"mcp_servers": null}}), &selected),
    Ok(Vec::new())
  );
  assert_eq!(
    servers_to_turn_off(&json!({}), &selected),
    Err(McpInventoryError::MissingConfiguration)
  );
}

/// A server the step does not choose must show up turned off; anything else refuses the step.
#[test]
fn only_the_chosen_servers_run() {
  let selected = BTreeSet::from(["docs".to_owned()]);
  let turned_off = json!({"data": [
    {"name": "docs", "runtimeStatus": "connected"},
    {"name": "pencil", "runtimeStatus": "disabled"}
  ]});
  assert_eq!(verify_only_chosen_run(&turned_off, &selected), Ok(()));
  for status in ["connected", "starting", "failed"] {
    let running = json!({"data": [{"name": "cua_repl", "runtimeStatus": status}]});
    assert_eq!(
      verify_only_chosen_run(&running, &selected),
      Err(McpInventoryError::UnchosenServer("cua_repl".to_owned()))
    );
  }
}

/// Hooks would run programs of the user's setup with the agent's work: a configuration with any
/// refuses the step.
#[test]
fn user_hooks_refuse_the_step() {
  for hooks in [json!(null), json!({}), json!([])] {
    assert_eq!(refuse_user_hooks(&json!({"config": {"hooks": hooks}})), Ok(()));
  }
  assert_eq!(refuse_user_hooks(&json!({"config": {}})), Ok(()));
  let hooks = json!({"config": {"hooks": {"PreToolUse": [{"command": "audit.exe"}]}}});
  assert_eq!(refuse_user_hooks(&hooks), Err(McpInventoryError::UserHooks));
}
