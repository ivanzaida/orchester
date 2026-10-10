//! Widenings shared by the profile editor and the domain tools.
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  id::ResourceId,
  permission::{AgentPermissions, widening::Widening},
  resource::AgentProfile,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::Message;

/// Compared with the latest published version, never an unapproved draft.
pub fn reasons(
  db: &ProjectDb,
  id: Option<ResourceId>,
  profile: &AgentProfile,
  provider: AgentProvider,
) -> Result<Vec<Message>, StoreError> {
  let latest = id
    .map(|id| db.latest_version::<AgentProfile>(id))
    .transpose()?
    .flatten();
  let base = latest.as_ref().map_or_else(
    || AgentPermissions::most_restrictive(provider, Platform::current()),
    |version| version.body().permissions().clone(),
  );
  let mut reasons: Vec<_> = profile
    .permissions()
    .widenings_from(&base)
    .into_iter()
    .map(message)
    .collect();
  if profile.mcp_servers().ids().iter().any(|server| {
    !latest
      .as_ref()
      .is_some_and(|version| version.body().mcp_servers().ids().contains(server))
  }) {
    reasons.push(Message::new("appMcp.widens.mcpServers"));
  }
  Ok(reasons)
}

pub fn message(widening: Widening) -> Message {
  Message::new(match widening {
    Widening::Repository => "appMcp.widens.repository",
    Widening::WritableFolder => "appMcp.widens.writableFolder",
    Widening::Commands => "appMcp.widens.commands",
    Widening::Network => "appMcp.widens.network",
    Widening::ToolCallsUnasked => "appMcp.widens.toolCallsUnasked",
    Widening::AutoApprove => "appMcp.widens.autoApprove",
  })
}
