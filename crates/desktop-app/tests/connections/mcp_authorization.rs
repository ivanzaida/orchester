//! A remote MCP server's authorization as a person sets it in Add MCP server: each scheme with its fields, the
//! secret it sends chosen from the project's secrets, what Save says while one is missing, and what the inspector
//! shows of it (the secret by its name, never its value).

use orchester_app::routes::NewConnection;
use orchester_core::{
  connection::mcp::{BasicUsername, McpAuthorization, McpHeaderName, McpTransport},
  id::SecretId,
  store::SecretStore,
};

use super::{api_provider::choose, harness::Seeded};

/// The seeded `SENTRY_TOKEN`: its id and its label in a secret select.
fn sentry_token(seeded: &Seeded) -> (SecretId, String) {
  let secret = seeded
    .db()
    .secrets()
    .expect("secrets read")
    .into_iter()
    .find(|secret| secret.name().as_str() == "SENTRY_TOKEN")
    .expect("the seeded secret");
  let label = format!("{} ({})", secret.name(), secret.environment().as_str());
  (secret.id(), label)
}

/// Adds remote server `name` with authorization `scheme` (its option's words), `extra` typed into the field the
/// scheme asks for, and the seeded secret; and confirms the save, which widens.
fn add_remote(seeded: &mut Seeded, name: &str, scheme: &str, extra: Option<(&str, &str)>) {
  let (_, secret) = sentry_token(seeded);
  seeded.add(NewConnection::McpServer);
  seeded.set("connection-mcp-name", name);
  seeded.click("connection-mcp-http");
  seeded.set("connection-mcp-url", "https://mcp.example.dev/mcp");
  choose(seeded, "connection-mcp-authorization", scheme);
  if let Some((id, value)) = extra {
    seeded.set(id, value);
  }
  seeded.click("connection-save");
  assert!(
    seeded
      .text("connection-form-reason")
      .contains("choose the secret to send"),
    "{scheme}: Save says the secret is missing"
  );
  assert!(seeded.exists("connection-mcp-secret-error"));
  choose(seeded, "connection-mcp-secret", &secret);
  assert!(!seeded.exists("connection-form-reason"), "{scheme}: it can be saved");
  seeded.click("connection-save");
  seeded.until("the question", |seeded| {
    seeded.exists("connection-confirm-save-confirm")
  });
  seeded.click("connection-confirm-save-confirm");
  seeded.until("the saved server", |seeded| {
    if let Some(reason) = seeded.window.texts("connection-form-reason") {
      panic!("not saved: {reason:?}");
    }
    seeded.exists("connection-inspector")
  });
}

fn authorization(seeded: &Seeded, name: &str) -> McpAuthorization {
  let server = seeded.server(name).expect("the server is stored");
  let McpTransport::Http(http) = server.transport() else {
    panic!("a remote server");
  };
  http.authorization().clone()
}

#[test]
fn connections_mcp_remote_server_is_saved_with_each_authorization() {
  let mut seeded = Seeded::new("connections");
  let (token, label) = sentry_token(&seeded);
  add_remote(
    &mut seeded,
    "basic-docs",
    "Basic: user name and password",
    Some(("connection-mcp-username", "deploy")),
  );
  assert_eq!(
    authorization(&seeded, "basic-docs"),
    McpAuthorization::Basic {
      username: BasicUsername::new("deploy").unwrap(),
      password: token,
    }
  );
  assert!(
    seeded
      .text("connection-authentication")
      .contains(&format!("Basic, user deploy, password from {label}"))
  );
  add_remote(&mut seeded, "pasted-docs", "Basic: pasted credential", None);
  assert_eq!(
    authorization(&seeded, "pasted-docs"),
    McpAuthorization::BasicCredential { credential: token }
  );
  add_remote(
    &mut seeded,
    "keyed-docs",
    "Custom header",
    Some(("connection-mcp-header", "X-API-Key")),
  );
  assert_eq!(
    authorization(&seeded, "keyed-docs"),
    McpAuthorization::Header {
      name: McpHeaderName::new("X-API-Key").unwrap(),
      value: token,
    }
  );
  assert!(
    seeded
      .text("connection-authentication")
      .contains(&format!("Header X-API-Key from {label}"))
  );
  add_remote(&mut seeded, "bearer-docs", "Bearer token", None);
  assert_eq!(
    authorization(&seeded, "bearer-docs"),
    McpAuthorization::Bearer { token }
  );
}

#[test]
fn connections_mcp_authorization_fields_say_why_they_cannot_be_used() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::McpServer);
  seeded.set("connection-mcp-name", "docs");
  seeded.click("connection-mcp-http");
  seeded.set("connection-mcp-url", "https://mcp.example.dev/mcp");
  assert!(
    !seeded.exists("connection-mcp-secret"),
    "no authorization asks for no secret"
  );
  choose(&mut seeded, "connection-mcp-authorization", "Custom header");
  seeded.set("connection-mcp-header", "Content-Type");
  assert!(
    seeded
      .text("connection-mcp-header-note")
      .contains("The agent's CLI sets Content-Type itself")
  );
  choose(
    &mut seeded,
    "connection-mcp-authorization",
    "Basic: user name and password",
  );
  seeded.set("connection-mcp-username", "a:b");
  assert!(
    seeded
      .text("connection-mcp-username-note")
      .contains("cannot contain a colon")
  );
  assert!(!seeded.exists("connection-mcp-header"), "Basic asks for no header");
}

/// Edit MCP server opens a stored bearer server with its scheme and secret as they are, and saving it unchanged
/// keeps it a bearer server.
#[test]
fn connections_mcp_stored_bearer_server_opens_as_it_was() {
  let mut seeded = Seeded::new("connections");
  let sentry = seeded.server("sentry").expect("sentry");
  let before = authorization(&seeded, "sentry");
  assert!(matches!(before, McpAuthorization::Bearer { .. }));
  seeded.go(orchester_app::routes::ProjectPage::EditConnection(sentry.id()));
  seeded.until("the form", |seeded| seeded.exists("connection-mcp-authorization"));
  assert!(seeded.text("connection-mcp-authorization").contains("Bearer token"));
  assert_eq!(seeded.text("connection-form-status"), "No changes");
  assert!(seeded.text("connection-mcp-secret").contains("SENTRY_TOKEN"));
}
