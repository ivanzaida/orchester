//! The team MCP server a team run's bundle pins.

use orchester_core::{
  connection::AgentProvider,
  id::{ResourceId, VersionId},
  resource::{PromptText, ProviderSettings, TeamAgentProfile, TeamMcpServer, Version, VersionNumber},
};

use crate::support::{at, model, name};

/// A team MCP server a team profile chooses, and the pinned version of the server.
pub fn pinned_server() -> (ResourceId, TeamAgentProfile, Version<TeamMcpServer>) {
  let mcp_server = ResourceId::generate();
  let server_body: TeamMcpServer = serde_json::from_value(serde_json::json!({
    "name": "docs",
    "launch": {"kind": "program", "program": "docs-mcp"}
  }))
  .expect("a team MCP server");
  let team = TeamAgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    (AgentProvider::CodexCli, model(), ProviderSettings::default()),
  )
  .with_mcp_servers(vec![mcp_server])
  .expect("distinct servers");
  let server_version = Version::new(VersionId::generate(), VersionNumber::FIRST, server_body, at(5));
  (mcp_server, team, server_version)
}
