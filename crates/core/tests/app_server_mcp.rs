use std::collections::BTreeSet;

use orchester_core::connection::app_server_mcp::{
  McpInventoryError, UnavailableServer, unavailable_selected, verify_selected_servers,
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
