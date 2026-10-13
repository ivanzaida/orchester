//! *Connect an agent* (`NzOSY`): the commands for the address the settings hold, the token masked, the server named
//! `orchester`; and the commands themselves as Copy puts them on the clipboard.

use std::num::NonZeroU16;

use orchester_app::mcp::connect::{self, ClaudeScope};

use super::*;

#[test]
fn settings_mcp_connect_shows_the_commands_with_the_token_masked() {
  let port = free_port();
  let mut shown = Shown::installed(port, Arc::default());
  shown.until("listening", |shown| shown.says("settings-mcp-state", "Listening"));
  let claude = format!(
    "claude mcp add --transport http --scope user orchester http://127.0.0.1:{port}/mcp --header \"Authorization: \
     Bearer ••••••••••••••••\""
  );
  assert!(shown.says("settings-mcp-connect-claude-command", &claude));
  shown.click("settings-mcp-connect-codex");
  shown.until("the Codex tab", |shown| {
    shown.texts("settings-mcp-connect-codex-command").is_some()
  });
  assert!(shown.says(
    "settings-mcp-connect-codex-command",
    &format!(
      "codex mcp add orchester --url http://127.0.0.1:{port}/mcp --bearer-token-env-var ORCHESTER_APP_MCP_TOKEN"
    )
  ));
  assert!(shown.says(
    "settings-mcp-connect-codex-variable",
    &connect::codex_variable("••••••••••••••••")
  ));
  assert!(shown.texts("settings-mcp-connect-codex-config").is_some());
  let token = shown.token();
  assert!(!shown.everything().iter().any(|text| text.contains(&token)));
}

#[test]
fn app_mcp_connect_commands_register_orchester_with_the_token() {
  let port = NonZeroU16::new(47821).expect("a port");
  assert_eq!(
    connect::claude(port, ClaudeScope::User, "t0ken"),
    "claude mcp add --transport http --scope user orchester http://127.0.0.1:47821/mcp --header \"Authorization: \
     Bearer t0ken\""
  );
  assert_eq!(
    connect::claude(port, ClaudeScope::Local, "t0ken"),
    "claude mcp add --transport http --scope local orchester http://127.0.0.1:47821/mcp --header \"Authorization: \
     Bearer t0ken\""
  );
  assert_eq!(
    connect::codex(port),
    "codex mcp add orchester --url http://127.0.0.1:47821/mcp --bearer-token-env-var ORCHESTER_APP_MCP_TOKEN"
  );
  assert_eq!(
    connect::codex_config(port, "t0ken"),
    "[mcp_servers.orchester]\nurl = \"http://127.0.0.1:47821/mcp\"\nhttp_headers = { Authorization = \"Bearer t0ken\" }"
  );
  let variable = connect::codex_variable("t0ken");
  if cfg!(target_os = "macos") {
    assert_eq!(variable, "launchctl setenv ORCHESTER_APP_MCP_TOKEN t0ken");
  } else {
    assert_eq!(variable, "setx ORCHESTER_APP_MCP_TOKEN t0ken");
  }
  assert_eq!(connect::SERVER_NAME, "orchester", "never the runs' Orchester");
}
