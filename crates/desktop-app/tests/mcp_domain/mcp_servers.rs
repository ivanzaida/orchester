//! The MCP server tools: adding, changing, turning on and off and removing a project's MCP servers, as
//! Connections › MCP servers does. What widens what agents may do (a new server, a new name, another program, a
//! variable but a harmless one, a variable that takes a secret, a tool that runs without asking, turning a server
//! on) needs a person's switch, judged before anything reads the disk; dropping exempt tools, turning off and
//! removing never do. A program is a fixed file outside the repository, and a secret is named, never its value.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::SecretId,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::SecretStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::Served;

/// Kontur seeded empty, with its Codex connection, so profiles can choose servers; and its repository.
fn served() -> (Served, PathBuf) {
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  let (served, seeded) = Served::seeded_with("empty", Some(codex));
  (served, seeded.repo)
}

/// A program outside the project's repository, to stand for a server's: this test's own.
fn program() -> String {
  std::env::current_exe()
    .expect("the test program")
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

/// The servers `list_connections` lists.
fn listed(served: &mut Served) -> Vec<Value> {
  served.ok("list_connections", &json!({ "project": "Kontur" }))["mcp_servers"]
    .as_array()
    .expect("a list of servers")
    .clone()
}

/// A secret of the project, `name` in Development.
fn add_secret(repo: &Path, name: &str) {
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new(name).expect("a secret name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Development,
    },
    Timestamp::now(),
  );
  ProjectDb::open(repo)
    .expect("the database opens")
    .create_secret(&secret)
    .expect("the secret is stored");
}

#[test]
fn domain_mcp_servers_widen_only_with_a_person_and_narrow_always() {
  let (mut served, repo) = served();
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
  assert_eq!(
    server["environment"],
    json!([{ "name": "RUST_LOG", "kind": "text" }]),
    "a plain value is not listed"
  );
  served.let_automation_widen(false);
  let narrowed = served.ok(
    "update_mcp_server",
    &change(json!({ "exempt_tools": ["get_screenshot"], "environment": [] })),
  );
  assert_eq!(narrowed["server"]["exempt_tools"], json!(["get_screenshot"]));
  add_secret(&repo, "PENCIL_TOKEN");
  for widening in [
    json!({ "exempt_tools": ["get_screenshot", "batch_design"] }),
    json!({ "args": ["--app", "web"] }),
    json!({ "environment": [{ "name": "NODE_OPTIONS", "value": "--require evil" }] }),
    json!({ "environment": [{ "name": "LANG", "value": "${secret:PENCIL_TOKEN@development}" }] }),
    json!({ "working_directory": "tools" }),
    json!({ "name": "pencil" }),
    // Judged by its text before anything reads it: a share is not contacted.
    json!({ "program": r"\\server\share\mcp.exe" }),
  ] {
    assert_person_only(&served.refused("update_mcp_server", &change(widening)));
  }
  let off = json!({ "project": "Kontur", "server": "pencil-design", "enabled": false });
  assert_eq!(served.ok("set_mcp_server_enabled", &off)["server"]["enabled"], false);
  let on = json!({ "project": "Kontur", "server": "pencil-design", "enabled": true });
  assert_person_only(&served.refused("set_mcp_server_enabled", &on));
  served.let_automation_widen(true);
  assert_eq!(served.ok("set_mcp_server_enabled", &on)["server"]["enabled"], true);
  served.let_automation_widen(false);
  served.ok(
    "remove_mcp_server",
    &json!({ "project": "Kontur", "server": "pencil-design" }),
  );
  assert!(listed(&mut served).is_empty());
}

#[test]
fn domain_mcp_server_fields_are_checked_as_the_page_checks_them() {
  let (mut served, repo) = served();
  served.let_automation_widen(true);
  let refused = |served: &mut Served, field: &str, value: Value| {
    let mut args = pencil();
    args[field] = value;
    served.refused("add_mcp_server", &args)["code"].clone()
  };
  assert_eq!(
    refused(&mut served, "program", json!("tools/mcp-server")),
    "program_in_worktree"
  );
  let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-program.exe");
  assert_eq!(refused(&mut served, "program", json!(missing)), "program_not_found");
  std::fs::write(repo.join("server.exe"), b"").expect("a file in the repository");
  let inside = repo.join("server.exe");
  assert_eq!(refused(&mut served, "program", json!(inside)), "program_in_repository");
  #[cfg(windows)]
  assert_eq!(
    refused(&mut served, "program", json!(r"\\server\share\mcp.exe")),
    "program_not_local"
  );
  assert_eq!(
    refused(&mut served, "url", json!("https://mcp.example.dev/mcp")),
    "invalid_transport"
  );
  assert_eq!(refused(&mut served, "name", json!("Pencil Design")), "invalid_server");
  assert_eq!(
    refused(&mut served, "name", json!("a1b2c3d4-e5f6-4789-8abc-def012345678")),
    "invalid_server",
    "a name that reads as an id"
  );
  assert_eq!(refused(&mut served, "args", json!(["--app", ""])), "invalid_server");
  let secret = json!([{ "name": "TOKEN", "value": format!("${{secret:{}}}", SecretId::generate()) }]);
  assert_eq!(
    refused(&mut served, "environment", secret),
    "invalid_server",
    "only the project's secrets"
  );
  let remote = json!({
    "project": "Kontur", "name": "sentry", "url": "https://mcp.sentry.dev/mcp?token=abc&org=kontur",
    "args": ["--verbose"],
  });
  assert_eq!(
    served.refused("add_mcp_server", &remote)["code"],
    "field_not_applicable"
  );
  let remote =
    json!({ "project": "Kontur", "name": "sentry", "url": "https://mcp.sentry.dev/mcp?token=abc&org=kontur" });
  let added = served.ok("add_mcp_server", &remote);
  assert_eq!(added["server"]["transport"]["kind"], "http");
  assert_eq!(
    added["server"]["transport"]["url"],
    "https://mcp.sentry.dev/mcp?token=…&org=…"
  );
  let stored = served.ok("add_mcp_server", &pencil());
  assert_eq!(
    stored["server"]["transport"]["program"],
    json!(program()),
    "the program is the file itself"
  );
  assert_eq!(served.refused("add_mcp_server", &pencil())["code"], "name_taken");
  let id = added["server"]["id"].as_str().expect("an id").to_owned();
  let by_id = served.ok(
    "set_mcp_server_enabled",
    &json!({ "project": "Kontur", "server": id, "enabled": false }),
  );
  assert_eq!(by_id["server"]["name"], "sentry");
}

/// A remote server's authorization round-trips by scheme, its secret named and never taken by value; another scheme
/// that sends a secret widens, dropping it narrows.
#[test]
fn domain_mcp_server_authorization_takes_each_scheme_by_secret_name() {
  let (mut served, repo) = served();
  add_secret(&repo, "SENTRY_TOKEN");
  served.let_automation_widen(true);
  let basic = json!({ "scheme": "basic", "username": "deploy", "secret": "SENTRY_TOKEN@development" });
  let remote = json!({
    "project": "Kontur", "name": "sentry", "url": "https://mcp.sentry.dev/mcp", "authorization": basic,
  });
  let added = served.ok("add_mcp_server", &remote);
  assert_eq!(added["server"]["transport"]["authorization"], basic);
  let sentry = |fields: Value| {
    let mut args = json!({ "project": "Kontur", "server": "sentry" });
    if let (Value::Object(args), Value::Object(fields)) = (&mut args, fields) {
      args.extend(fields);
    }
    args
  };
  let header = json!({ "scheme": "header", "name": "X-API-Key", "secret": "SENTRY_TOKEN@development" });
  let changed = served.ok("update_mcp_server", &sentry(json!({ "authorization": header })));
  assert_eq!(changed["server"]["transport"]["authorization"], header);
  let pasted = json!({ "scheme": "basic_credential", "secret": "SENTRY_TOKEN@development" });
  served.ok("update_mcp_server", &sentry(json!({ "authorization": pasted })));
  assert_eq!(listed(&mut served)[0]["transport"]["authorization"], pasted);
  served.let_automation_widen(false);
  let bearer = json!({ "scheme": "bearer", "secret": "SENTRY_TOKEN@development" });
  assert_person_only(&served.refused("update_mcp_server", &sentry(json!({ "authorization": bearer }))));
  let none = served.ok(
    "update_mcp_server",
    &sentry(json!({ "authorization": { "scheme": "none" } })),
  );
  assert_eq!(
    none["server"]["transport"]["authorization"],
    json!({ "scheme": "none" }),
    "dropping it narrows"
  );
  served.let_automation_widen(true);
  let by_value = json!({ "authorization": { "scheme": "bearer", "secret": "s3cr3t-value" } });
  let refused = served.refused("update_mcp_server", &sentry(by_value));
  assert_eq!(refused["code"], "invalid_server", "{refused}");
  let reserved =
    json!({ "authorization": { "scheme": "header", "name": "Host", "secret": "SENTRY_TOKEN@development" } });
  assert_eq!(
    served.refused("update_mcp_server", &sentry(reserved))["code"],
    "invalid_server"
  );
  served.ok("add_mcp_server", &pencil());
  let on_a_program = change(json!({ "authorization": { "scheme": "none" } }));
  assert_eq!(
    served.refused("update_mcp_server", &on_a_program)["code"],
    "field_not_applicable"
  );
}

#[test]
fn domain_mcp_servers_a_profile_chooses_are_not_removed() {
  let (mut served, _) = served();
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

#[test]
fn domain_profile_mcp_selection_checks_enabled_added_disabled_kept_removed_and_missing() {
  let (mut served, repo) = served();
  served.let_automation_widen(true);
  served.ok("add_mcp_server", &pencil());
  let mut args = json!({
    "project": "Kontur", "role": "Designer", "connection": "Codex · Kontur", "model": "gpt-5.5",
    "mcp_servers": ["pencil-design"],
  });
  served.ok("save_profile_draft", &args);
  args["profile"] = json!("Designer");
  let off = json!({"project": "Kontur", "server": "pencil-design", "enabled": false});
  served.ok("set_mcp_server_enabled", &off);
  served.ok("save_profile_draft", &args);
  args["mcp_servers"] = json!([]);
  served.ok("save_profile_draft", &args);
  args["mcp_servers"] = json!(["pencil-design"]);
  let error = served.refused("save_profile_draft", &args);
  assert_eq!(error["code"], "mcp_server_not_offered");
  assert!(
    error["reasons"][0].as_str().expect("reason").contains("turned off"),
    "{error}"
  );
  args["mcp_servers"] = json!(["missing-server"]);
  let error = served.refused("save_profile_draft", &args);
  assert_eq!(error["code"], "mcp_server_not_found");
  let db = ProjectDb::open(&repo).expect("fixture db");
  let listed = orchester_workspace::agents::data::ProjectAgents::load(&db).expect("agents");
  let designer = listed
    .profiles
    .iter()
    .find(|entry| entry.profile.role().as_str() == "Designer")
    .expect("designer");
  assert!(
    designer.profile.mcp_servers().ids().is_empty(),
    "rejections changed no stored selection"
  );
}
