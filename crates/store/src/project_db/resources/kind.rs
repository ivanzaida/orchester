//! How a resource kind is written in the `resources` table.

use orchester_core::resource::ResourceKind;

use crate::{StoreError, rows::corrupt};

pub(in crate::project_db) const fn kind_text(kind: ResourceKind) -> &'static str {
  match kind {
    ResourceKind::AgentProfile => "agent_profile",
    ResourceKind::Instruction => "instruction",
    ResourceKind::Pipeline => "pipeline",
    ResourceKind::Tool => "tool",
    ResourceKind::Skill => "skill",
    ResourceKind::TestSuite => "test_suite",
    ResourceKind::Service => "service",
    ResourceKind::Stack => "stack",
    ResourceKind::TeamReviewer => "team_reviewer",
    ResourceKind::McpServer => "mcp_server",
  }
}

pub(in crate::project_db) fn parse_kind(value: &str) -> Result<ResourceKind, StoreError> {
  match value {
    "agent_profile" => Ok(ResourceKind::AgentProfile),
    "instruction" => Ok(ResourceKind::Instruction),
    "pipeline" => Ok(ResourceKind::Pipeline),
    "tool" => Ok(ResourceKind::Tool),
    "skill" => Ok(ResourceKind::Skill),
    "test_suite" => Ok(ResourceKind::TestSuite),
    "service" => Ok(ResourceKind::Service),
    "stack" => Ok(ResourceKind::Stack),
    "team_reviewer" => Ok(ResourceKind::TeamReviewer),
    "mcp_server" => Ok(ResourceKind::McpServer),
    other => Err(corrupt("resource kind", other)),
  }
}
