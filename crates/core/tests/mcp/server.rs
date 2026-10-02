use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  connection::mcp::{
    HttpServer, McpEnvValue, McpEnvironment, McpEnvironmentError, McpServer, McpServerError, McpServerName,
    McpServerNameError, McpTransport, McpUrl, StdioServer, TransportError, WorkingDirectory,
  },
  id::{ConnectionId, SecretId},
  resource::{CommandArg, CommandProgram, VariableName},
  time::Timestamp,
};

fn name(value: &str) -> McpServerName {
  McpServerName::new(value).expect("valid name")
}

fn stdio(program: &str) -> McpTransport {
  McpTransport::Stdio(StdioServer::new(
    CommandProgram::new(program).expect("valid program"),
    vec![CommandArg::new("--stdio").expect("valid argument")],
    None,
  ))
}

fn variables(entries: &[(&str, McpEnvValue)]) -> BTreeMap<VariableName, McpEnvValue> {
  entries
    .iter()
    .map(|(name, value)| (VariableName::new(name).expect("valid name"), value.clone()))
    .collect()
}

#[test]
fn mcp_server_names_are_keys_both_clis_accept() {
  assert_eq!(name("  project-knowledge ").as_str(), "project-knowledge");
  assert_eq!(name("docs_2").as_str(), "docs_2");
  assert_eq!(McpServerName::new(" "), Err(McpServerNameError::Blank));
  for invalid in ["Project", "2docs", "-docs", "docs.search", "docs search", "dökumente"] {
    assert_eq!(
      McpServerName::new(invalid),
      Err(McpServerNameError::Invalid),
      "{invalid}"
    );
  }
  assert_eq!(McpServerName::new(&"a".repeat(49)), Err(McpServerNameError::TooLong));
  assert!(serde_json::from_str::<McpServerName>(r#""Bad Name""#).is_err());
}

#[test]
fn mcp_urls_are_http_without_credentials() {
  let url = McpUrl::new(" https://mcp.example.dev/v1?team=a ").expect("valid URL");
  assert_eq!(url.as_str(), "https://mcp.example.dev/v1?team=a");
  assert_eq!(url.host(), "mcp.example.dev");
  assert_eq!(
    McpUrl::new("http://localhost:8931/mcp").expect("valid").host(),
    "localhost:8931"
  );
  assert_eq!(McpUrl::new(""), Err(TransportError::BlankUrl));
  for invalid in ["ftp://example.dev", "example.dev/mcp", "https://", "https://:80/mcp"] {
    assert_eq!(McpUrl::new(invalid), Err(TransportError::UrlScheme), "{invalid}");
  }
  assert_eq!(
    McpUrl::new("https://user:token@mcp.example.dev/"),
    Err(TransportError::UrlCredentials)
  );
  assert!(matches!(
    McpUrl::new("https://mcp.example.dev/a b"),
    Err(TransportError::Whitespace(_))
  ));
  // A backslash before the query could end the host where the check does not; after it, it is data.
  for invalid in [r"https://mcp.example.dev\@evil.example/", r"http://localhost\mcp"] {
    assert_eq!(McpUrl::new(invalid), Err(TransportError::Backslash), "{invalid}");
  }
  for invalid in [
    "http://[::1]x/mcp",
    "http://[::1/mcp",
    "https://[]/mcp",
    "https://[mcp.dev]/mcp",
  ] {
    assert_eq!(McpUrl::new(invalid), Err(TransportError::InvalidHost), "{invalid}");
  }
}

/// URLs the check accepted before the backslash and IPv6 rules: a stored server keeps loading.
#[test]
fn mcp_urls_still_accepted() {
  for url in [
    "https://пример.рф/mcp",
    "https://bücher.de/mcp",
    "https://例え.テスト/mcp",
    "https://हिन्दी.भारत/mcp",
    "https://mcp.example.dev:8443/v1?team=a&scope=read",
    "https://mcp_server.example.dev/mcp",
    "https://mcp.example.dev./mcp",
    "HTTPS://MCP.EXAMPLE.DEV/MCP",
    "http://[::ffff:127.0.0.1]:8931/mcp",
    "http://[::1]:8931/mcp",
    "http://[fe80::1%25eth0]:8931/mcp",
    "https://mcp.example.dev:/mcp",
    "https://xn--e1afmkfd.xn--p1ai/mcp",
    "https://%6Dcp.example.dev/mcp",
    r"https://mcp.example.dev/mcp?path=C:\work",
    r"https://mcp.example.dev/mcp#C:\work",
  ] {
    assert!(McpUrl::new(url).is_ok(), "{url}");
  }
}

#[test]
fn mcp_working_directories_stay_in_the_worktree_unless_absolute() {
  let inside = WorkingDirectory::new("tools/docs-server").expect("inside the worktree");
  assert_eq!(
    inside.resolve(Path::new("/repo/worktree")),
    Path::new("/repo/worktree").join("tools/docs-server")
  );
  assert!(matches!(
    WorkingDirectory::new("../outside"),
    Err(TransportError::OutsideWorktree(_))
  ));
  assert!(matches!(
    WorkingDirectory::new(r"tools\..\..\outside"),
    Err(TransportError::OutsideWorktree(_))
  ));
  assert_eq!(WorkingDirectory::new("  "), Err(TransportError::BlankWorkingDirectory));
  let absolute = if cfg!(windows) {
    r"C:\servers\docs"
  } else {
    "/opt/servers/docs"
  };
  assert_eq!(
    WorkingDirectory::new(absolute)
      .expect("absolute")
      .resolve(Path::new("/repo/worktree")),
    Path::new(absolute)
  );
}

#[test]
fn mcp_environment_names_are_distinct_and_values_have_no_nul() {
  let secret = SecretId::generate();
  let environment = McpEnvironment::new(variables(&[
    ("DOCS_TOKEN", McpEnvValue::Secret(secret)),
    ("OTHER_TOKEN", McpEnvValue::Secret(secret)),
    ("LOG_LEVEL", McpEnvValue::Text("debug".to_owned())),
  ]))
  .expect("valid variables");
  assert_eq!(environment.secrets().collect::<Vec<_>>(), vec![secret]);
  assert_eq!(
    McpEnvironment::new(variables(&[
      ("token", McpEnvValue::Text("a".to_owned())),
      ("TOKEN", McpEnvValue::Text("b".to_owned())),
    ])),
    Err(McpEnvironmentError::Duplicate("token".to_owned()))
  );
  assert_eq!(
    McpEnvironment::new(variables(&[("A", McpEnvValue::Text("x\0y".to_owned()))])),
    Err(McpEnvironmentError::Nul("A".to_owned()))
  );
  let json = serde_json::to_string(&environment).unwrap();
  assert!(json.contains(&secret.to_string()), "{json}");
  assert_eq!(serde_json::from_str::<McpEnvironment>(&json).unwrap(), environment);
}

#[test]
fn mcp_servers_validate_through_their_constructor() {
  let token = SecretId::generate();
  let environment = McpEnvironment::new(variables(&[("DOCS_TOKEN", McpEnvValue::Secret(token))])).expect("valid");
  let http = McpTransport::Http(HttpServer::new(
    McpUrl::new("https://mcp.example.dev/").expect("valid"),
    Some(token),
  ));
  assert_eq!(
    McpServer::new(
      ConnectionId::generate(),
      name("docs"),
      http.clone(),
      environment.clone(),
      Timestamp::from_unix_millis(1)
    ),
    Err(McpServerError::EnvironmentOnHttp)
  );
  let remote = McpServer::new(
    ConnectionId::generate(),
    name("docs"),
    http,
    McpEnvironment::default(),
    Timestamp::from_unix_millis(1),
  )
  .expect("an HTTP server without variables");
  assert_eq!(remote.secrets(), vec![token]);
  let mut local = McpServer::new(
    ConnectionId::generate(),
    name("files"),
    stdio("npx"),
    environment,
    Timestamp::from_unix_millis(1),
  )
  .expect("a stdio server");
  assert!(local.enabled());
  local.set_enabled(false);
  assert!(!local.enabled());
  assert_eq!(local.secrets(), vec![token]);
  let transport = McpTransport::Http(HttpServer::new(
    McpUrl::new("http://localhost:9/").expect("valid"),
    None,
  ));
  assert_eq!(
    local.update(name("files"), transport, local.environment().clone()),
    Err(McpServerError::EnvironmentOnHttp)
  );
  assert_eq!(local.transport(), &stdio("npx"), "a refused update changes nothing");
}

#[test]
fn mcp_transports_round_trip_through_json() {
  let transport = McpTransport::Stdio(StdioServer::new(
    CommandProgram::new("node").expect("valid"),
    vec![CommandArg::new("server.js").expect("valid")],
    Some(WorkingDirectory::new("tools").expect("valid")),
  ));
  let json = serde_json::to_string(&transport).unwrap();
  assert_eq!(serde_json::from_str::<McpTransport>(&json).unwrap(), transport);
  let tampered = json.replace(r#""tools""#, r#""../outside""#);
  assert!(serde_json::from_str::<McpTransport>(&tampered).is_err());
}
