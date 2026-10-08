use std::collections::BTreeSet;

use orchester_core::connection::app_server_mcp::{
  McpInventoryError, UnavailableServer, refuse_user_hooks, servers_to_turn_off, unavailable_selected,
  verify_only_chosen_run, verify_selected_servers,
};
use serde_json::json;

/// Codex's reasons as they are, for checks that need no redaction.
fn kept(text: &str) -> String {
  text.to_owned()
}

#[test]
fn selected_server_is_checked_and_extra_global_servers_remain_available() {
  let selected = BTreeSet::from(["project-pencil".to_owned()]);
  let inventory = json!({"data":[
    {"name":"global-docs","runtimeStatus":"connected","toolsError":null},
    {"name":"project-pencil","runtimeStatus":"connected","toolsError":null},
    {"name":"global-other","runtimeStatus":"failed","toolsError":"not needed"}
  ],"nextCursor":null});
  assert_eq!(verify_selected_servers(&inventory, &selected, kept), Ok(()));
  let missing = json!({"data":[{"name":"global-docs","runtimeStatus":"connected"}],"nextCursor":null});
  let error = verify_selected_servers(&missing, &selected, kept).expect_err("missing");
  assert_eq!(error, McpInventoryError::MissingSelected("project-pencil".to_owned()));
  assert_eq!(
    error.to_string(),
    "Codex App Server did not load the MCP server `project-pencil` selected by this agent profile"
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
    assert!(
      matches!(
        verify_selected_servers(&inventory, &selected, kept),
        Err(McpInventoryError::UnavailableSelected(UnavailableServer { ref name, .. })) if name == "project-pencil"
      ),
      "{inventory}"
    );
  }
}

/// The step that needs its servers says which one is not connected, and why, as Codex lists it; Codex's
/// reason reaches the error only as `redact` left it.
#[test]
fn a_selected_server_that_is_not_connected_is_named_with_its_reason() {
  let selected = BTreeSet::from(["design".to_owned(), "Orchester".to_owned()]);
  let inventory = json!({"data":[
    {"name":"Orchester","runtimeStatus":"connected","toolsError":null},
    {"name":"design","runtimeStatus":"failed","toolsError":"MCP startup failed: token s3cret refused"}
  ],"nextCursor":null});
  let redact = |text: &str| text.replace("s3cret", "<redacted>");
  let error = verify_selected_servers(&inventory, &selected, redact).expect_err("not connected");
  assert_eq!(
    error.to_string(),
    "The MCP server `design` selected by this agent profile is not connected in Codex App Server: \
     failed: MCP startup failed: token <redacted> refused"
  );
  assert!(!format!("{error:?}").contains("s3cret"), "{error:?}");
}

/// A step that does not ask before MCP actions works without the selected servers that are not
/// connected, each named with its state and Codex's reason.
#[test]
fn selected_servers_that_are_not_connected_are_named_for_the_activity() {
  let selected = BTreeSet::from(["docs".to_owned(), "missing".to_owned(), "pencil".to_owned()]);
  let inventory = json!({"data":[
    {"name":"docs","runtimeStatus":"failed","toolsError":"private diagnostic"},
    {"name":"pencil","runtimeStatus":"connected","toolsError":null},
    {"name":"global-other","runtimeStatus":"failed","toolsError":null}
  ],"nextCursor":null});
  assert_eq!(
    unavailable_selected(&inventory, &selected, kept),
    Ok(vec![
      UnavailableServer {
        name: "missing".to_owned(),
        state: None,
        error: None
      },
      UnavailableServer {
        name: "docs".to_owned(),
        state: Some("failed".to_owned()),
        error: Some("private diagnostic".to_owned())
      },
    ])
  );
  assert_eq!(
    unavailable_selected(&json!({"data":[],"nextCursor":"more"}), &selected, kept),
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

/// Codex 0.159.2 runs a server whose name has a dot, but a dotted key cannot name it: `mcp_servers.a.b.enabled`
/// refuses the thread, and `docs.x` would add a setting to the chosen `docs`. The step stops before its thread.
#[test]
fn a_configured_server_whose_name_has_a_dot_refuses_the_step() {
  let selected = BTreeSet::from(["docs".to_owned()]);
  for dotted in ["a.b", "docs.x"] {
    let config = json!({"config": {"mcp_servers": {"docs": {"enabled": true}, dotted: {"enabled": true}}}});
    let error = servers_to_turn_off(&config, &selected).expect_err("refused");
    assert_eq!(error, McpInventoryError::DottedName(dotted.to_owned()));
    assert_eq!(
      error.to_string(),
      format!(
        "Your own Codex configuration has an MCP server named `{dotted}`; Orchester cannot keep a server whose \
         name has a dot from its agents, so rename it"
      )
    );
  }
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
