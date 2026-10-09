//! The person's decision the setup tools respect: automation may narrow what agents may do, but widening
//! it (broader profile permissions or MCP servers, a CLI program of its own choice, environment variables
//! that change what programs load) is refused as `person_only` unless a person turned on *Let automation
//! change agent permissions* in the window. No tool changes that switch.

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  id::ResourceId,
  permission::{AgentPermissions, widening::Widening},
  resource::AgentProfile,
  store::ResourceStore,
};
use orchester_store::ProjectDb;

use crate::{
  app_mcp::{error::ToolError, services::Services},
  messages::Message,
};

/// Refused unless nothing widens or a person lets automation widen; `reasons` say what would widen.
pub(super) fn allowed(services: &Services, mut reasons: Vec<Message>) -> Result<(), ToolError> {
  if reasons.is_empty() || services.workspace.automation_changes_permissions()? {
    return Ok(());
  }
  reasons.insert(0, Message::new("appMcp.errors.personOnly"));
  Err(ToolError::refused("person_only", reasons))
}

/// What `profile` widens beyond the latest published version of profile `id`, or, before its first
/// version, beyond the most restrictive permissions `provider` enforces here and no MCP server.
pub(super) fn of_profile(
  db: &ProjectDb,
  id: Option<ResourceId>,
  profile: &AgentProfile,
  provider: AgentProvider,
) -> Result<Vec<Message>, ToolError> {
  let latest = match id {
    Some(id) => db.latest_version::<AgentProfile>(id)?,
    None => None,
  };
  let (base, servers) = latest.as_ref().map_or_else(
    || {
      (
        AgentPermissions::most_restrictive(provider, Platform::current()),
        Vec::new(),
      )
    },
    |version| {
      let body = version.body();
      (body.permissions().clone(), body.mcp_servers().ids().to_vec())
    },
  );
  let mut reasons: Vec<Message> = profile
    .permissions()
    .widenings_from(&base)
    .into_iter()
    .map(message)
    .collect();
  if profile
    .mcp_servers()
    .ids()
    .iter()
    .any(|server| !servers.contains(server))
  {
    reasons.push(Message::new("appMcp.widens.mcpServers"));
  }
  Ok(reasons)
}

fn message(widening: Widening) -> Message {
  Message::new(match widening {
    Widening::Repository => "appMcp.widens.repository",
    Widening::WritableFolder => "appMcp.widens.writableFolder",
    Widening::Commands => "appMcp.widens.commands",
    Widening::Network => "appMcp.widens.network",
    Widening::ToolCallsUnasked => "appMcp.widens.toolCallsUnasked",
    Widening::AutoApprove => "appMcp.widens.autoApprove",
  })
}
