//! A team agent profile's version sends the references its body names, in one order (`docs/cloud-api.md`, "Team
//! resource references"): the CLI kind, the MCP servers, tools, skills and instructions as team resources, then the
//! secrets by name.

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  name::Name,
  resource::{ModelId, PromptText, ProviderSettings, TeamAgentProfile},
  secret::SecretName,
};
use serde_json::json;

fn profile() -> TeamAgentProfile {
  TeamAgentProfile::new(
    Name::new("Builder").expect("valid role"),
    String::new(),
    PromptText::new("Build it.").expect("valid prompt"),
    (
      AgentProvider::ClaudeCodeCli,
      ModelId::new("claude-sonnet-5-5").expect("valid model"),
      ProviderSettings::default(),
    ),
  )
}

#[test]
fn team_profile_references_are_the_cli_kind_alone_for_a_profile_that_uses_nothing() {
  assert_eq!(
    ReferenceTarget::of_profile(&profile()),
    [ReferenceTarget::CliKind {
      cli: AgentProvider::ClaudeCodeCli
    }]
  );
}

#[test]
fn team_profile_references_follow_the_body_in_one_order_and_serialize_as_the_contract_says() {
  let (docs, build, skill, tone) = (
    ResourceId::generate(),
    ResourceId::generate(),
    ResourceId::generate(),
    ResourceId::generate(),
  );
  let profile = profile()
    .with_secrets(vec![SecretName::new("STAGING_TOKEN").expect("name")])
    .and_then(|profile| profile.with_instruction_resources(vec![tone]))
    .and_then(|profile| profile.with_skills(vec![skill]))
    .and_then(|profile| profile.with_tools(vec![build]))
    .and_then(|profile| profile.with_mcp_servers(vec![docs]))
    .expect("distinct lists");

  let references = ReferenceTarget::of_profile(&profile);

  assert_eq!(
    serde_json::to_value(&references).expect("serializes"),
    json!([
      {"type": "cli_kind", "cli": "claude_code_cli"},
      {"type": "team_resource", "kind": "mcp_server", "id": docs},
      {"type": "team_resource", "kind": "tool", "id": build},
      {"type": "team_resource", "kind": "skill", "id": skill},
      {"type": "team_resource", "kind": "instruction", "id": tone},
      {"type": "secret_name", "name": "STAGING_TOKEN"},
    ])
  );
  assert!(references.contains(&ReferenceTarget::TeamResource {
    kind: SharedResourceKind::McpServer,
    id: docs
  }));
}
