//! The MCP server tools in a team project, where an MCP server is the team's resource and only an admin changes it: a
//! member is refused `not_admin` before anything is looked at; a new server and a version that widens what agents may
//! do are a person's unless they let automation change agent permissions; a path or a value, which the team body
//! cannot hold, is refused with a code of its own (a key in a URL's query too); an update of the program's name widens
//! like any other change; a remove reads what
//! uses the server first and archives nothing while that cannot be read. These tests run without a team server (the
//! project's link names none), so what reaches the server is `workspace`'s `team_mcp` tests.

use orchester_core::{
  resource::{ResourceKind, TeamMcpServer},
  sharing::MemberRole,
  store::ResourceStore,
};
use serde_json::{Value, json};

use super::team_fill_ins::{Team, team_as};

fn npx(name: &str) -> Value {
  json!({ "project": "Kontur", "name": name, "program": "npx", "args": ["-y", "@acme/docs"] })
}

fn change(fields: Value) -> Value {
  let mut args = json!({ "project": "Kontur", "server": "knowledge" });
  if let (Value::Object(args), Value::Object(fields)) = (&mut args, fields) {
    args.extend(fields);
  }
  args
}

/// How many team MCP servers the project offers.
fn servers(team: &Team) -> usize {
  team.db().resources(ResourceKind::McpServer).expect("resources").len()
}

/// The version of `knowledge` this computer has published last.
fn latest(team: &Team) -> u32 {
  team
    .db()
    .latest_version::<TeamMcpServer>(team.program_server)
    .expect("read")
    .expect("a version")
    .number()
    .get()
}

#[test]
fn domain_team_mcp_server_tools_refuse_a_member_as_not_admin() {
  let mut team = team_as(MemberRole::Member);
  team.served.let_automation_widen(true);
  let changes = team.connection_changes();
  let calls = [
    ("add_mcp_server", npx("docs")),
    ("update_mcp_server", change(json!({ "args": ["--other"] }))),
    (
      "remove_mcp_server",
      json!({ "project": "Kontur", "server": "knowledge" }),
    ),
  ];
  for (tool, arguments) in calls {
    let refused = team.served.refused(tool, &arguments);
    assert_eq!(refused["code"], "not_admin", "{tool}: {refused}");
  }
  assert_eq!(servers(&team), 1, "nothing was created or archived");
  assert_eq!(latest(&team), 1);
  assert_eq!(team.connection_changes(), changes, "nothing was counted as changed");
}

#[test]
fn domain_team_mcp_server_add_is_a_persons_unless_automation_may_widen() {
  let mut team = team_as(MemberRole::Owner);
  team.served.let_automation_widen(false);
  let refused = team.served.refused("add_mcp_server", &npx("docs"));
  assert_eq!(refused["code"], "person_only", "{refused}");
  assert_eq!(servers(&team), 1, "nothing was created");

  // Let through the gate, it reaches the team server; this project's link names none, so the new server is taken back.
  team.served.let_automation_widen(true);
  let refused = team.served.refused("add_mcp_server", &npx("docs"));
  assert_eq!(refused["code"], "team_server_not_reached", "{refused}");
  assert_eq!(
    servers(&team),
    1,
    "a server the team server did not take is not created"
  );
  assert!(team.db().waiting_publishes().expect("the outbox").is_empty());
}

#[test]
fn domain_team_mcp_server_add_refuses_what_the_team_body_cannot_hold() {
  let mut team = team_as(MemberRole::Owner);
  team.served.let_automation_widen(true);
  let with = |fields: Value| {
    let mut args = npx("docs");
    if let (Value::Object(args), Value::Object(fields)) = (&mut args, fields) {
      args.extend(fields);
    }
    args
  };
  let cases = [
    (with(json!({ "program": "/usr/local/bin/npx" })), "team_path_refused"),
    (with(json!({ "program": r"C:\tools\npx.exe" })), "team_path_refused"),
    (with(json!({ "working_directory": "/srv/tools" })), "team_path_refused"),
    (
      with(json!({ "environment": [{ "name": "TOKEN", "value": "s3cret" }] })),
      "team_value_refused",
    ),
    (
      with(json!({ "authorization": { "scheme": "bearer", "secret": "TOKEN@production" } })),
      "team_value_refused",
    ),
    (
      with(json!({ "secret_headers": [{ "header": "X-API-Key", "secret": "API_KEY" }] })),
      "field_not_applicable",
    ),
    (with(json!({ "variable_names": ["TOKEN", "token"] })), "invalid_server"),
    (with(json!({ "variable_names": ["TOKEN", "TOKEN"] })), "invalid_server"),
    (with(json!({ "variable_names": ["lowercase"] })), "invalid_server"),
    (with(json!({ "name": "knowledge" })), "name_taken"),
    (with(json!({ "name": "Not A Name" })), "invalid_server"),
    (
      json!({ "project": "Kontur", "name": "issues", "url": "https://mcp.example.org", "variable_names": ["TOKEN"] }),
      "field_not_applicable",
    ),
  ];
  for (arguments, code) in cases {
    let refused = team.served.refused("add_mcp_server", &arguments);
    assert_eq!(refused["code"], code, "{arguments}: {refused}");
  }
  assert_eq!(servers(&team), 1, "nothing was created");
}

#[test]
fn domain_team_mcp_server_refuses_a_key_in_the_url_and_says_where_a_secret_goes() {
  let mut team = team_as(MemberRole::Owner);
  team.served.let_automation_widen(true);
  let remote = |url: &str| json!({ "project": "Kontur", "name": "issues", "url": url });
  for url in [
    "https://mcp.example.org/x?key=sk-live-123",
    "https://mcp.example.org/x#key=sk-live-123",
  ] {
    let refused = team.served.refused("add_mcp_server", &remote(url));
    assert_eq!(refused["code"], "team_value_refused", "{url}: {refused}");
    assert!(refused.to_string().contains("secret_headers"), "{refused}");
    assert!(
      !refused.to_string().contains("sk-live-123"),
      "the key is not echoed: {refused}"
    );
  }
  let switched = change(json!({ "url": "https://mcp.example.org/x?key=sk-live-123" }));
  let refused = team.served.refused("update_mcp_server", &switched);
  assert_eq!(refused["code"], "team_value_refused", "{refused}");
  assert!(!refused.to_string().contains("sk-live-123"), "{refused}");
  assert_eq!(servers(&team), 1, "nothing was created");
  assert_eq!(latest(&team), 1, "nothing was published");

  let plain = team
    .served
    .refused("add_mcp_server", &remote("https://mcp.example.org/x"));
  assert_eq!(
    plain["code"], "team_server_not_reached",
    "a clean URL gets as far as the team server: {plain}"
  );
}

#[test]
fn domain_team_mcp_server_update_publishes_the_next_version_when_it_changes_something() {
  let mut team = team_as(MemberRole::Owner);
  team.served.let_automation_widen(false);

  // Nothing differs: nothing is published.
  let same = team
    .served
    .ok("update_mcp_server", &change(json!({ "exempt_tools": [] })));
  assert_eq!(same["unchanged"], true, "{same}");
  assert_eq!(latest(&team), 1);

  // Dropping nothing and adding variable names widens nothing, so it is taken, kept here as v2 and, with no team
  // server to number it, waits to be sent.
  let named = team
    .served
    .ok("update_mcp_server", &change(json!({ "variable_names": ["TOKEN"] })));
  assert_eq!(named["server"]["published_version"], 2, "{named}");
  assert_eq!(named["server"]["launch"]["variable_names"], json!(["TOKEN"]), "{named}");
  assert_eq!(
    named["server"]["waiting_for_server"]["code"], "team_server_unreachable",
    "{named}"
  );
  assert_eq!(latest(&team), 2);
  assert!(!named.to_string().contains("s3cret"));

  // A name stays the one it was made with.
  let renamed = team
    .served
    .refused("update_mcp_server", &change(json!({ "name": "other" })));
  assert_eq!(renamed["code"], "name_not_editable", "{renamed}");
  // The same name is no change.
  team
    .served
    .ok("update_mcp_server", &change(json!({ "name": "knowledge" })));

  // Another endpoint kind, a remote server where a program ran, widens.
  let remote = change(json!({ "url": "https://mcp.example.org/x" }));
  assert_eq!(team.served.refused("update_mcp_server", &remote)["code"], "person_only");
  assert_eq!(latest(&team), 2, "a refused change publishes nothing");
}

#[test]
fn domain_team_mcp_server_update_that_widens_is_a_persons_unless_automation_may_widen() {
  let mut team = team_as(MemberRole::Owner);
  team.served.let_automation_widen(false);
  for fields in [
    json!({ "args": ["--root", "/"] }),
    json!({ "exempt_tools": ["delete_everything"] }),
  ] {
    let refused = team.served.refused("update_mcp_server", &change(fields.clone()));
    assert_eq!(refused["code"], "person_only", "{fields}: {refused}");
    assert!(
      refused["reasons"].as_array().is_some_and(|reasons| reasons.len() > 1),
      "{refused}"
    );
  }
  assert_eq!(latest(&team), 1, "nothing was published");

  team.served.let_automation_widen(true);
  let published = team
    .served
    .ok("update_mcp_server", &change(json!({ "args": ["--root", "/"] })));
  assert_eq!(published["server"]["published_version"], 2, "{published}");
  assert_eq!(
    published["server"]["launch"]["args"],
    json!(["--root", "/"]),
    "{published}"
  );
}

#[test]
fn domain_team_mcp_server_remove_archives_nothing_while_what_uses_it_cannot_be_read() {
  let mut team = team_as(MemberRole::Owner);
  team.served.let_automation_widen(true);
  let changes = team.connection_changes();
  let refused = team.served.refused(
    "remove_mcp_server",
    &json!({ "project": "Kontur", "server": "knowledge" }),
  );
  assert_eq!(refused["code"], "team_unavailable", "{refused}");
  assert_eq!(servers(&team), 1);
  assert_eq!(team.connection_changes(), changes);
}
