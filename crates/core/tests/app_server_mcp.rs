use std::collections::BTreeSet;

use orchester_core::connection::app_server_mcp::{McpInventoryError, verify_selected_servers};
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
