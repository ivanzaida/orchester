//! A remote server's authorization: each scheme as the stored server keeps it, the header each CLI is given, and
//! that no secret value reaches an argument, a plain variable or a `Debug` output.

use std::convert::Infallible;

use orchester_core::{
  connection::{
    invocation::{ClaudeMcpConfig, CodexMcp, HttpAuthorization},
    mcp::{
      BasicUsername, HttpServer, McpAuthorization, McpAuthorizationError, McpEnvironment, McpHeaderName, McpServer,
      McpServerName, McpTransport, McpUrl,
    },
  },
  id::{ConnectionId, SecretId},
  time::Timestamp,
};
use serde_json::json;

use super::invocation::{assert_hidden, codex_command, remote, secret};

const PASSWORD: &str = "p@ss:w0rd";
/// `deploy:p@ss:w0rd` in base64.
const ENCODED: &str = "ZGVwbG95OnBAc3M6dzByZA==";
const VARIABLE: &str = "ORCHESTER_MCP_TOKEN_PROJECT_KNOWLEDGE";

/// `authorization` as an invocation sends it, its one secret's value `value`.
fn read(authorization: &McpAuthorization, value: &str) -> HttpAuthorization {
  HttpAuthorization::read(authorization, |_| Ok::<_, Infallible>(secret(value))).expect("the secret is read")
}

fn basic() -> McpAuthorization {
  McpAuthorization::Basic {
    username: BasicUsername::new("deploy").expect("valid"),
    password: SecretId::generate(),
  }
}

/// What Codex is given for one remote server sent `authorization`: the override naming its header and variable,
/// and the variable's value.
fn codex(authorization: HttpAuthorization) -> (String, String) {
  let (mcp, left_out) = CodexMcp::new(&[remote(authorization)]);
  assert!(left_out.is_empty(), "{left_out:?}");
  let header = mcp
    .args()
    .into_iter()
    .find(|arg| arg.starts_with("mcp_servers.project-knowledge.") && !arg.contains(".url="))
    .expect("the server's authorization");
  let secrets = mcp.secrets();
  assert_eq!(secrets.len(), 1);
  assert_eq!(secrets[0].name(), VARIABLE);
  (header, secrets[0].expose().to_owned())
}

/// The `headers` Claude Code's configuration file gives the remote server sent `authorization`.
fn claude_headers(authorization: HttpAuthorization) -> serde_json::Value {
  let (config, _) = ClaudeMcpConfig::new(&[remote(authorization)]);
  let config = config.expect("the server is kept");
  let file: serde_json::Value = serde_json::from_str(config.contents()).expect("JSON");
  file["mcpServers"]["project-knowledge"]["headers"].clone()
}

#[test]
fn mcp_authorization_basic_sends_the_encoded_user_and_password_to_both_clis() {
  let sent = read(&basic(), PASSWORD);
  let (header, value) = codex(sent.clone());
  assert_eq!(
    header,
    format!(r#"mcp_servers.project-knowledge.env_http_headers={{"Authorization"="{VARIABLE}"}}"#)
  );
  assert_eq!(value, format!("Basic {ENCODED}"));
  assert_eq!(
    claude_headers(sent.clone()),
    json!({ "Authorization": format!("Basic {ENCODED}") })
  );
  let (mcp, _) = CodexMcp::new(&[remote(sent.clone())]);
  assert_hidden(&codex_command(&mcp), &[PASSWORD, ENCODED]);
  let (config, _) = ClaudeMcpConfig::new(&[remote(sent.clone())]);
  let shown = format!("{:?} {sent:?} {:?}", config.expect("kept"), remote(sent.clone()));
  assert!(!shown.contains(PASSWORD) && !shown.contains(ENCODED), "{shown}");
}

#[test]
fn mcp_authorization_pasted_basic_credential_is_sent_with_its_scheme_once() {
  let pasted = McpAuthorization::BasicCredential {
    credential: SecretId::generate(),
  };
  for (value, header) in [
    (ENCODED, format!("Basic {ENCODED}")),
    (&format!("Basic {ENCODED}"), format!("Basic {ENCODED}")),
    (&format!("basic {ENCODED}"), format!("basic {ENCODED}")),
  ] {
    assert_eq!(codex(read(&pasted, value)).1, header);
    assert_eq!(claude_headers(read(&pasted, value)), json!({ "Authorization": header }));
  }
}

#[test]
fn mcp_authorization_custom_header_goes_by_its_name() {
  let custom = McpAuthorization::Header {
    name: McpHeaderName::new("X-API-Key").expect("valid"),
    value: SecretId::generate(),
  };
  let (header, value) = codex(read(&custom, "k3y"));
  assert_eq!(
    header,
    format!(r#"mcp_servers.project-knowledge.env_http_headers={{"X-API-Key"="{VARIABLE}"}}"#)
  );
  assert_eq!(value, "k3y");
  assert_eq!(claude_headers(read(&custom, "k3y")), json!({ "X-API-Key": "k3y" }));
}

#[test]
fn mcp_authorization_bearer_and_none_reach_the_clis_as_before() {
  let bearer = McpAuthorization::Bearer {
    token: SecretId::generate(),
  };
  let (header, value) = codex(read(&bearer, "t0ken"));
  assert_eq!(
    header,
    format!(r#"mcp_servers.project-knowledge.bearer_token_env_var="{VARIABLE}""#)
  );
  assert_eq!(value, "t0ken");
  assert_eq!(
    claude_headers(read(&bearer, "t0ken")),
    json!({ "Authorization": "Bearer t0ken" })
  );
  let (mcp, _) = CodexMcp::new(&[remote(read(&McpAuthorization::None, "unused"))]);
  assert!(mcp.secrets().is_empty());
  assert!(
    !mcp
      .args()
      .iter()
      .any(|arg| arg.contains("header") || arg.contains("bearer"))
  );
  assert_eq!(claude_headers(HttpAuthorization::None), serde_json::Value::Null);
}

#[test]
fn mcp_authorization_that_cannot_read_its_secret_fails() {
  let failed = HttpAuthorization::read(&basic(), |_| Err("no value"));
  assert_eq!(failed.err(), Some("no value"));
}

fn url() -> McpUrl {
  McpUrl::new("https://mcp.example.dev/v1").expect("valid")
}

#[test]
fn mcp_authorization_keeps_a_stored_bearer_server_as_it_was() {
  let token = SecretId::generate();
  let stored = json!({ "kind": "http", "url": "https://mcp.example.dev/v1", "bearer": token.to_string() });
  let bearer = McpTransport::Http(HttpServer::new(url(), McpAuthorization::Bearer { token }));
  assert_eq!(serde_json::from_value::<McpTransport>(stored.clone()).unwrap(), bearer);
  assert_eq!(serde_json::to_value(&bearer).unwrap(), stored);
  let before = json!({ "kind": "http", "url": "https://mcp.example.dev/v1", "bearer": null });
  let none = McpTransport::Http(HttpServer::new(url(), McpAuthorization::None));
  assert_eq!(serde_json::from_value::<McpTransport>(before).unwrap(), none);
}

#[test]
fn mcp_authorization_round_trips_each_scheme_and_refuses_a_tampered_one() {
  let header = McpAuthorization::Header {
    name: McpHeaderName::new("X-API-Key").expect("valid"),
    value: SecretId::generate(),
  };
  let pasted = McpAuthorization::BasicCredential {
    credential: SecretId::generate(),
  };
  for authorization in [basic(), pasted, header] {
    let transport = McpTransport::Http(HttpServer::new(url(), authorization));
    let json = serde_json::to_string(&transport).unwrap();
    assert!(!json.contains("\"bearer\""), "{json}");
    assert_eq!(serde_json::from_str::<McpTransport>(&json).unwrap(), transport);
  }
  let tampered = json!({ "kind": "http", "url": "https://mcp.example.dev/v1",
    "authorization": { "scheme": "header", "name": "Host", "value": SecretId::generate().to_string() } });
  assert!(serde_json::from_value::<McpTransport>(tampered).is_err());
  let colon = json!({ "kind": "http", "url": "https://mcp.example.dev/v1",
    "authorization": { "scheme": "basic", "username": "a:b", "password": SecretId::generate().to_string() } });
  assert!(serde_json::from_value::<McpTransport>(colon).is_err());
}

#[test]
fn mcp_authorization_names_are_checked() {
  assert_eq!(BasicUsername::new(" deploy ").unwrap().as_str(), "deploy");
  assert_eq!(BasicUsername::new(" "), Err(McpAuthorizationError::BlankUsername));
  assert_eq!(BasicUsername::new("a:b"), Err(McpAuthorizationError::UsernameColon));
  assert_eq!(BasicUsername::new("a\nb"), Err(McpAuthorizationError::UsernameControl));
  assert_eq!(
    BasicUsername::new(&"u".repeat(BasicUsername::MAX_CHARS + 1)),
    Err(McpAuthorizationError::UsernameTooLong)
  );
  assert_eq!(McpHeaderName::new("X-API-Key").unwrap().as_str(), "X-API-Key");
  assert_eq!(McpHeaderName::new("Authorization").unwrap().as_str(), "Authorization");
  assert_eq!(McpHeaderName::new(""), Err(McpAuthorizationError::BlankHeader));
  assert_eq!(
    McpHeaderName::new("X API"),
    Err(McpAuthorizationError::HeaderInvalid("X API".to_owned()))
  );
  assert_eq!(
    McpHeaderName::new("mcp-session-id"),
    Err(McpAuthorizationError::HeaderReserved("mcp-session-id".to_owned()))
  );
  assert_eq!(
    McpHeaderName::new(&"h".repeat(McpHeaderName::MAX_CHARS + 1)),
    Err(McpAuthorizationError::HeaderTooLong)
  );
}

#[test]
fn mcp_authorization_secret_is_the_servers_secret() {
  let value = SecretId::generate();
  let header = McpAuthorization::Header {
    name: McpHeaderName::new("X-API-Key").expect("valid"),
    value,
  };
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("docs").expect("valid"),
    McpTransport::Http(HttpServer::new(url(), header)),
    McpEnvironment::default(),
    Timestamp::from_unix_millis(1),
  )
  .expect("valid");
  assert_eq!(server.secrets(), vec![value]);
  assert_eq!(McpAuthorization::None.secret(), None);
}
