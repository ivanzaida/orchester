//! Widenings shared by the profile editor and the domain tools.
use orchester_core::{
  connection::{AgentProvider, invocation::Platform, moves_to_another_account},
  id::ResourceId,
  permission::{AgentPermissions, widening::Widening},
  resource::AgentProfile,
  store::{ConnectionStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::Message;

/// Compared with the latest published version, never an unapproved draft. A profile whose connection now runs on
/// another account than the latest version's widens too ([`moves_to_another_account`]).
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
  let account = |connection| {
    Ok::<_, StoreError>(
      db.connection(connection)?
        .map(|connection| connection.account().clone()),
    )
  };
  let before = match &latest {
    Some(version) => account(version.body().connection())?,
    None => None,
  };
  if let Some(after) = account(profile.connection())?
    && moves_to_another_account(before.as_ref(), &after)
  {
    reasons.push(Message::new("appMcp.widens.account"));
  }
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
    Widening::Browser => "appMcp.widens.browser",
    Widening::BrowserSite => "appMcp.widens.browserSite",
  })
}
