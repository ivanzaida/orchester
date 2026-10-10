//! The MCP server tools: adding, changing, turning on and off and removing a project's MCP servers, as
//! Connections › MCP servers does. What widens what agents may do (a new server, another program, a variable
//! but a harmless one, a tool that runs without asking, turning a server on) needs a person's switch;
//! renaming, dropping exempt tools, turning off and removing never do. A program is an absolute path to an
//! existing file, and a secret is named, never its value.

use std::path::Path;

use serde_json::{Value, json};

use super::Served;

/// Kontur seeded empty, with its Codex connection, so profiles can choose servers.
fn served() -> Served {
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  Served::seeded_with("empty", Some(codex)).0
}

/// An existing file to stand for a server's program.
fn program() -> String {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("build.rs")
    .to_string_lossy()
    .into_owned()
}

fn pencil() -> Value {
  json!({
    "project": "Kontur",
    "name": "pencil-design",
    "program": program(),
    "args": ["--app", "desktop", "--agent", "codexCLI"],
    "environment": [{ "name": "RUST_LOG", "value": "info" }],
    "exempt_tools": ["get_editor_state", "get_screenshot", "batch_get"],
  })
}

fn change(fields: Value) -> Value {
  let mut args = json!({ "project": "Kontur", "server": "pencil-design" });
  if let (Value::Object(args), Value::Object(fields)) = (&mut args, fields) {
    args.extend(fields);
  }
  args
}

fn assert_person_only(error: &Value) {
  assert_eq!(error["code"], "person_only", "{error}");
  assert!(
    error["reasons"].as_array().is_some_and(|reasons| reasons.len() > 1),
    "{error}"
  );
}

/// The servers `list_connections` lists, by name.
fn listed(served: &mut Served) -> Vec<Value> {
  served.ok("list_connections", &json!({ "project": "Kontur" }))["mcp_servers"]
    .as_array()
    .expect("a list of servers")
    .clone()
}

#[test]
fn domain_mcp_servers_widen_only_with_a_person_and_narrow_always() {
  let mut served = served();
  assert_person_only(&served.refused("add_mcp_server", &pencil()));
  assert!(listed(&mut served).is_empty());
  served.let_automation_widen(true);
  let added = served.ok("add_mcp_server", &pencil());
  let server = &added["server"];
  assert_eq!(server["enabled"], true);
  assert_eq!(server["transport"]["kind"], "stdio");
  assert_eq!(
    server["transport"]["args"],
    json!(["--app", "desktop", "--agent", "codexCLI"])
  );
  assert_eq!(server["exempt_tools"].as_array().map(Vec::len), Some(3));
  assert_eq!(listed(&mut served)[0]["name"], "pencil-design");
  served.let_automation_widen(false);
  let narrowed = served.ok(
    "update_mcp_server",
    &change(json!({ "exempt_tools": ["get_screenshot"], "environment": [] })),
  );
  assert_eq!(narrowed["server"]["exempt_tools"], json!(["get_screenshot"]));
  for widening in [
    json!({ "exempt_tools": ["get_screenshot", "batch_design"] }),
    json!({ "args": ["--app", "web"] }),
    json!({ "environment": [{ "name": "NODE_OPTIONS", "value": "--require evil" }] }),
    json!({ "working_directory": "tools" }),
  ] {
    assert_person_only(&served.refused("update_mcp_server", &change(widening)));
  }
  let renamed = served.ok("update_mcp_server", &change(json!({ "name": "pencil" })));
  assert_eq!(renamed["server"]["name"], "pencil");
  let off = json!({ "project": "Kontur", "server": "pencil", "enabled": false });
  assert_eq!(served.ok("set_mcp_server_enabled", &off)["server"]["enabled"], false);
  let on = json!({ "project": "Kontur", "server": "pencil", "enabled": true });
  assert_person_only(&served.refused("set_mcp_server_enabled", &on));
  served.let_automation_widen(true);
  assert_eq!(served.ok("set_mcp_server_enabled", &on)["server"]["enabled"], true);
  served.let_automation_widen(false);
  served.ok("remove_mcp_server", &json!({ "project": "Kontur", "server": "pencil" }));
  assert!(listed(&mut served).is_empty());
}

#[test]
fn domain_mcp_server_fields_are_checked_as_the_page_checks_them() {
  let mut served = served();
  served.let_automation_widen(true);
  let mut relative = pencil();
  relative["program"] = json!("npx");
  assert_eq!(
    served.refused("add_mcp_server", &relative)["code"],
    "program_not_absolute"
  );
  let mut missing = pencil();
  missing["program"] = json!(Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-program.exe"));
  assert_eq!(served.refused("add_mcp_server", &missing)["code"], "program_not_found");
  let mut both = pencil();
  both["url"] = json!("https://mcp.example.dev/mcp");
  assert_eq!(served.refused("add_mcp_server", &both)["code"], "invalid_transport");
  let mut named = pencil();
  named["name"] = json!("Pencil Design");
  assert_eq!(served.refused("add_mcp_server", &named)["code"], "invalid_server");
  let mut empty_arg = pencil();
  empty_arg["args"] = json!(["--app", ""]);
  assert_eq!(served.refused("add_mcp_server", &empty_arg)["code"], "invalid_server");
  let remote = json!({
    "project": "Kontur", "name": "sentry", "url": "https://mcp.sentry.dev/mcp", "bearer_secret": "SENTRY_TOKEN@development",
  });
  assert_eq!(served.refused("add_mcp_server", &remote)["code"], "invalid_server");
  let remote = json!({ "project": "Kontur", "name": "sentry", "url": "https://mcp.sentry.dev/mcp" });
  let added = served.ok("add_mcp_server", &remote);
  assert_eq!(added["server"]["transport"]["kind"], "http");
  assert_eq!(added["server"]["transport"]["bearer_secret"], Value::Null);
  served.ok("add_mcp_server", &pencil());
  assert_eq!(served.refused("add_mcp_server", &pencil())["code"], "name_taken");
}

#[test]
fn domain_mcp_servers_a_profile_chooses_are_not_removed() {
  let mut served = served();
  served.let_automation_widen(true);
  served.ok("add_mcp_server", &pencil());
  served.ok(
    "save_profile_draft",
    &json!({
      "project": "Kontur", "role": "Designer", "connection": "Codex · Kontur", "model": "gpt-5.5",
      "mcp_servers": ["pencil-design"],
    }),
  );
  assert_eq!(listed(&mut served)[0]["used_by"], json!(["Designer"]));
  let refused = served.refused(
    "remove_mcp_server",
    &json!({ "project": "Kontur", "server": "pencil-design" }),
  );
  assert_eq!(refused["code"], "in_use", "{refused}");
  assert_eq!(listed(&mut served).len(), 1);
}
