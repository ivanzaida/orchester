//! What this app cannot run as the team defined it, and a team secret the keychain could not report, are missing
//! parts of readiness, not assumed fine (`docs/design/team-configuration.md`, "Readiness").

use orchester_core::{
  id::{ConnectionId, ResourceId},
  resource::TeamMcpServer,
  secret::SecretName,
  sharing::{Here, McpNeed, McpUnsupported, Missing, Needs, Readiness, readiness},
};
use serde_json::json;

fn secret(value: &str) -> SecretName {
  SecretName::new(value).expect("valid secret name")
}

/// A remote server whose headers take `secrets`, one header each.
fn remote_server(id: ResourceId, secrets: &[&str]) -> McpNeed {
  let headers: Vec<_> = secrets
    .iter()
    .enumerate()
    .map(|(index, secret)| json!({"header": format!("X-Key-{index}"), "secret": secret}))
    .collect();
  let body: TeamMcpServer = serde_json::from_value(json!({
    "name": "issues",
    "launch": {"kind": "remote", "url": "https://mcp.example.org/x", "secret_headers": headers}
  }))
  .expect("a team MCP server");
  McpNeed::of(id, &body)
}

#[test]
fn a_remote_server_with_several_secret_headers_is_unsupported_even_when_every_secret_is_set() {
  let server = ResourceId::generate();
  let here = Here::nothing()
    .with_connection(
      ConnectionId::generate(),
      orchester_core::connection::AgentProvider::ClaudeCodeCli,
    )
    .with_mcp_on(server)
    .with_secret(secret("ISSUES_KEY"))
    .with_secret(secret("ORG_KEY"));

  let several = Needs::mcp_server(&remote_server(server, &["ISSUES_KEY", "ORG_KEY"]));
  assert_eq!(
    readiness(&several, &here).missing(),
    [Missing::McpUnsupported {
      server,
      why: McpUnsupported::MoreThanOneSecretHeader
    }]
  );

  let one = Needs::mcp_server(&remote_server(server, &["ISSUES_KEY"]));
  assert_eq!(readiness(&one, &here), Readiness::READY);
}

#[test]
fn a_team_secret_the_keychain_could_not_report_is_unreadable_not_missing_and_a_value_wins() {
  let needs = Needs::tool(vec![secret("DB_URL")]);
  let unreadable = Here::nothing().with_secret_unreadable(secret("DB_URL"));
  assert_eq!(
    readiness(&needs, &unreadable).missing(),
    [Missing::SecretUnreadable { name: secret("DB_URL") }]
  );
  assert_eq!(
    readiness(&needs, &unreadable.with_secret(secret("DB_URL"))),
    Readiness::READY
  );
}
