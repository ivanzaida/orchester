use orchester_core::{
  resource::{ResourceBody, ResourceKind, TeamMcpLaunch, TeamMcpServer},
  secret::SecretName,
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
    "name":"issues","launch":{"kind":"remote","url":"https://mcp.example.org/x","secret_headers":[
      {"header":"X-API-Key","secret":"ISSUES_KEY"},
      {"header":"Authorization","secret":"ISSUES_TOKEN","prefix":"Bearer "}
    ]},
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
  let header = json!({"name":"r","launch":{"kind":"remote","url":"https://mcp.example.org","secret_headers":[
    {"header":"X-Key","secret":"A_KEY"},{"header":"x-key","secret":"B_KEY"}]}});
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

fn remote(headers: &Value) -> Value {
  json!({"name":"r","launch":{"kind":"remote","url":"https://mcp.example.org/x","secret_headers":headers}})
}

fn names(server: &TeamMcpServer) -> Vec<String> {
  server.secrets().iter().map(SecretName::to_string).collect()
}

#[test]
fn a_program_variable_takes_the_team_secret_of_its_own_name() {
  let mut body = program("npx");
  body["launch"]["variables"] = json!(["PENCIL_TOKEN", "API_KEY"]);
  assert_eq!(names(&parse(body).expect("body")), ["PENCIL_TOKEN", "API_KEY"]);
  // Valid variable names that are not secret names are refused as such; the rest never were variable names.
  for name in ["token", "Token", "_TOKEN"] {
    let mut body = program("npx");
    body["launch"]["variables"] = json!([name]);
    let error = parse(body).expect_err(name);
    assert!(error.to_string().contains("secret name"), "{name}: {error}");
  }
  for name in ["9TOKEN", "TOKEN-2"] {
    let mut body = program("npx");
    body["launch"]["variables"] = json!([name]);
    assert!(parse(body).is_err(), "{name}");
  }
  let mut none = program("npx");
  none["launch"].as_object_mut().expect("launch").remove("variables");
  assert!(names(&parse(none).expect("body")).is_empty());
}

#[test]
fn a_secret_header_names_its_secret_and_an_optional_prefix() {
  let server = parse(remote(&json!([
    {"header":"Authorization","secret":"GITHUB_TOKEN","prefix":"Bearer "},
    {"header":"X-Org","secret":"GITHUB_TOKEN"},
    {"header":"X-API-Key","secret":"OTHER_KEY"}
  ])))
  .expect("body");
  // Two headers on one secret ask for it once; the order is the headers'.
  assert_eq!(names(&server), ["GITHUB_TOKEN", "OTHER_KEY"]);
  let TeamMcpLaunch::Remote(launch) = server.launch() else {
    panic!("a remote server");
  };
  let [first, second, _] = &launch.secret_headers()[..] else {
    panic!("three headers");
  };
  assert_eq!(first.prefix().map(|prefix| prefix.as_str()), Some("Bearer "));
  assert_eq!(second.prefix(), None);
  assert_eq!(first.secret().as_str(), "GITHUB_TOKEN");
  assert!(names(&parse(remote(&json!([]))).expect("no headers")).is_empty());
}

#[test]
fn a_secret_header_refuses_a_missing_or_invalid_secret_and_any_other_field() {
  for header in [
    json!({"header":"X-Key"}),
    json!({"header":"X-Key","secret":"lower"}),
    json!({"header":"X-Key","secret":""}),
    json!({"secret":"X_KEY"}),
    json!({"header":"Content-Type","secret":"X_KEY"}),
    json!({"header":"X-Key","secret":"X_KEY","value":"s3cret"}),
    json!({"header":"X-Key","secret":"X_KEY","id":"0191c7c0"}),
  ] {
    assert!(parse(remote(&json!([header.clone()]))).is_err(), "{header}");
  }
}

#[test]
fn a_header_prefix_is_short_plain_text() {
  for prefix in ["Bearer ", "Token ", "Basic ", "sk-"] {
    let header = json!({"header":"Authorization","secret":"X_KEY","prefix":prefix});
    assert!(parse(remote(&json!([header]))).is_ok(), "{prefix:?}");
  }
  for prefix in ["", "  ", "Bear\ner ", "Bearer\t", "x".repeat(33).as_str()] {
    let header = json!({"header":"Authorization","secret":"X_KEY","prefix":prefix});
    assert!(parse(remote(&json!([header]))).is_err(), "{prefix:?}");
  }
  let long = "sk-live-".to_owned() + &"1".repeat(40);
  let error = parse(remote(
    &json!([{"header":"Authorization","secret":"X_KEY","prefix":long}]),
  ))
  .expect_err("long");
  assert!(!error.to_string().contains("sk-live"), "{error}");
}

#[test]
fn a_body_with_bare_secret_header_names_is_refused_naming_the_header() {
  let error = parse(remote(&json!(["X-API-Key"]))).expect_err("the old shape");
  let message = error.to_string();
  assert!(message.contains("`X-API-Key` names no secret"), "{message}");
}
