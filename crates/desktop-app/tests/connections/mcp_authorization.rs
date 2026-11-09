//! A stored remote MCP server's authorization as Edit MCP server opens it. Making one with each scheme is the Agents
//! hub's form in a local project (`tests/agents/local_hub_create_remote.rs`).

use orchester_core::connection::mcp::{McpAuthorization, McpTransport};

use super::harness::Seeded;

fn authorization(seeded: &Seeded, name: &str) -> McpAuthorization {
  let server = seeded.server(name).expect("the server is stored");
  let McpTransport::Http(http) = server.transport() else {
    panic!("a remote server");
  };
  http.authorization().clone()
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
  assert_eq!(seeded.text(Seeded::REASON), "Nothing changed yet.");
  assert!(seeded.text("connection-mcp-secret").contains("SENTRY_TOKEN"));
}
