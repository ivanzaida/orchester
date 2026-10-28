use std::collections::BTreeSet;

use orchester_core::{
  connection::app_server_mcp::{
    McpInventoryError, UnavailableServer, refuse_user_hooks, servers_to_turn_off, thread_config, unavailable_selected,
    verify_only_chosen_run, verify_selected_servers,
  },
  orchester_tools::SERVER,
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
/// refuses the step while they are not turned off.
#[test]
fn user_hooks_refuse_the_step() {
  for hooks in [json!(null), json!({}), json!([])] {
    assert_eq!(refuse_user_hooks(&json!({"config": {"hooks": hooks}})), Ok(()));
  }
  assert_eq!(refuse_user_hooks(&json!({"config": {}})), Ok(()));
  let hooks = json!({"config": {"hooks": {"PreToolUse": [{"command": "audit.exe"}]}}});
  assert_eq!(refuse_user_hooks(&hooks), Err(McpInventoryError::UserHooks));
}

/// The `hooks` table Codex 0.159.2 reported for a person whose other agent tool installed global hooks
/// (2026-10-28): every event with an empty list, and the trust `state` of each `hooks.json` entry.
fn installed_hooks() -> serde_json::Value {
  let mut hooks: serde_json::Map<String, serde_json::Value> = [
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
  ]
  .into_iter()
  .map(|event| (event.to_owned(), json!([])))
  .collect();
  hooks.insert(
    "state".to_owned(),
    json!({
      r"C:\Users\me\.codex\hooks.json:session_start:0:0": {"trusted_hash": "sha256:5e1f"},
      r"C:\Users\me\.codex\hooks.json:pre_tool_use:0:0": {"trusted_hash": "sha256:9a0c"},
    }),
  );
  serde_json::Value::Object(hooks)
}

/// With `features.hooks = false`, which every Codex Orchester starts has, no hook runs, so the user's hooks no longer
/// refuse the step; without the flag, or with it on, they still do.
#[test]
fn user_hooks_turned_off_do_not_refuse_the_step() {
  let off = json!({"config": {"hooks": installed_hooks(), "features": {"hooks": false}}});
  assert_eq!(refuse_user_hooks(&off), Ok(()));
  let defined = json!({"config": {"hooks": {"PreToolUse": [{"command": "audit.exe"}]}, "features": {"hooks": false}}});
  assert_eq!(refuse_user_hooks(&defined), Ok(()), "none of them runs");
  for features in [
    json!(null),
    json!({}),
    json!({"hooks": true}),
    json!({"hooks": "false"}),
  ] {
    let on = json!({"config": {"hooks": installed_hooks(), "features": features}});
    assert_eq!(refuse_user_hooks(&on), Err(McpInventoryError::UserHooks), "{features}");
  }
  let unset = json!({"config": {"hooks": installed_hooks()}});
  assert_eq!(refuse_user_hooks(&unset), Err(McpInventoryError::UserHooks));
}

/// What `config/read` reports with the runs' relay [`SERVER`] from the process's overrides, a chosen `docs`, and the
/// app's own MCP server registered in the user's Codex as `orchester`, as `codex mcp add` writes it or with a header.
fn with_the_apps_server(orchester: &serde_json::Value) -> serde_json::Value {
  json!({"config": {"mcp_servers": {
    SERVER: {"command": "orchester-runner", "env_vars": ["ORCHESTER_TOOLS_TOKEN"]},
    "docs": {"command": "npx"},
    "orchester": orchester
  }}})
}

/// The app's own server `orchester` in the user's Codex configuration is turned off for the thread by its own
/// dotted key; the runs' relay `Orchester`, a different name to Codex, stays on.
#[test]
fn app_mcp_user_orchester_server_is_turned_off_beside_the_runs_relay() {
  let selected = BTreeSet::from([SERVER.to_owned(), "docs".to_owned()]);
  for orchester in [
    json!({"url": "http://127.0.0.1:47821/mcp", "bearer_token_env_var": "ORCHESTER_APP_MCP_TOKEN"}),
    json!({"url": "http://127.0.0.1:47821/mcp", "http_headers": {"Authorization": "Bearer app-token"}}),
  ] {
    let turned_off = servers_to_turn_off(&with_the_apps_server(&orchester), &selected).expect("no dotted name");
    assert_eq!(turned_off, ["orchester"]);
    let config = thread_config(&turned_off);
    assert_eq!(
      serde_json::Value::Object(config),
      json!({"mcp_servers.orchester.enabled": false})
    );
  }
}

/// The thread's inventory must show the app's server turned off; running, it refuses the step, while the relay runs.
#[test]
fn app_mcp_user_orchester_server_must_show_turned_off() {
  let selected = BTreeSet::from([SERVER.to_owned()]);
  let inventory = |status: &str| {
    json!({"data": [
      {"name": SERVER, "runtimeStatus": "connected"},
      {"name": "orchester", "runtimeStatus": status}
    ]})
  };
  assert_eq!(verify_only_chosen_run(&inventory("disabled"), &selected), Ok(()));
  for status in ["connected", "starting", "failed"] {
    assert_eq!(
      verify_only_chosen_run(&inventory(status), &selected),
      Err(McpInventoryError::UnchosenServer("orchester".to_owned()))
    );
  }
}
