use orchester_core::{
  resource::{ResourceBody, ResourceKind, TeamMcpLaunch, TeamMcpServer},
  sharing::{VersionWidening, widens},
};
use serde_json::{Value, json};

fn program(command: &str) -> Value {
  json!({"name":"knowledge","launch":{"kind":"program","program":command,"args":["-y"],"variables":["TOKEN"]}})
}

fn parse(body: Value) -> Result<TeamMcpServer, serde_json::Error> {
  serde_json::from_value(body)
}

#[test]
fn team_mcp_server_is_a_versioned_resource_and_round_trips() {
  assert_eq!(TeamMcpServer::KIND, ResourceKind::McpServer);
  let remote = json!({
    "name":"issues","launch":{"kind":"remote","url":"https://mcp.example.org/x","secret_headers":["X-API-Key"]},
    "exempt_tools":["search"]
  });
  for body in [program("npx"), remote] {
    let server = parse(body.clone()).expect("body");
    assert_eq!(serde_json::to_value(&server).expect("serialize"), body);
  }
  let server = parse(program("npx")).expect("body");
  let TeamMcpLaunch::Program(launch) = server.launch() else {
    panic!("a program");
  };
  assert_eq!(launch.program().as_str(), "npx");
  assert_eq!(launch.variables().len(), 1);
}

#[test]
fn team_mcp_server_refuses_a_path_a_value_or_a_local_id() {
  for path in [r"C:\tools\npx.exe", "/usr/bin/npx", "tools/run.sh"] {
    assert!(parse(program(path)).is_err(), "{path}");
  }
  let mut folder = program("npx");
  folder["launch"]["working_directory"] = json!("/srv/tools");
  assert!(parse(folder).is_err());
  let mut value = program("npx");
  value["launch"]["variables"] = json!({"TOKEN":"s3cret"});
  assert!(parse(value).is_err());
  for (field, extra) in [
    ("path", json!(r"C:\x")),
    ("env", json!({"A":"b"})),
    ("id", json!("0191c7c0")),
  ] {
    let mut body = program("npx");
    body[field] = extra.clone();
    assert!(parse(body).is_err(), "{field} at the top");
    let mut body = program("npx");
    body["launch"][field] = extra;
    assert!(parse(body).is_err(), "{field} in the launch");
  }
  let remote = json!({"name":"r","launch":{"kind":"remote","url":"https://u:p@mcp.example.org"}});
  assert!(parse(remote).is_err());
  let header =
    json!({"name":"r","launch":{"kind":"remote","url":"https://mcp.example.org","secret_headers":["X-Key","x-key"]}});
  assert!(parse(header).is_err());
  let mut twice = program("npx");
  twice["launch"]["variables"] = json!(["TOKEN", "token"]);
  assert!(parse(twice).is_err());
}

#[test]
fn team_mcp_server_widens_when_it_runs_or_reaches_more() {
  let before = parse(program("npx")).expect("before");
  assert!(widens(None, &before).is_empty());
  assert!(widens(Some(&before), &before).is_empty());
  let other = parse(program("uvx")).expect("other");
  assert!(matches!(
    widens(Some(&before), &other)[..],
    [VersionWidening::Command(_)]
  ));
  let mut exempt = program("npx");
  exempt["exempt_tools"] = json!(["delete"]);
  let exempt = parse(exempt).expect("exempt");
  assert_eq!(widens(Some(&before), &exempt), vec![VersionWidening::Access]);
  assert!(widens(Some(&exempt), &before).is_empty());
}

#[test]
fn team_mcp_server_refuses_a_url_with_a_query_or_a_fragment_without_naming_it() {
  for url in [
    "https://mcp.example.org/x?key=sk-live-123",
    "https://mcp.example.org/x?",
    "https://mcp.example.org/x#token=sk-live-123",
    "https://mcp.example.org/x?a=b#c",
  ] {
    let body = json!({"name":"r","launch":{"kind":"remote","url":url}});
    let error = parse(body).expect_err(url);
    let message = error.to_string();
    assert!(message.contains("secret header"), "{message}");
    assert!(!message.contains("sk-live-123"), "{message}");
    assert!(!format!("{error:?}").contains("sk-live-123"));
  }
  let plain = json!({"name":"r","launch":{"kind":"remote","url":"https://mcp.example.org/x/y"}});
  assert!(parse(plain).is_ok());
}
