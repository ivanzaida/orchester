//! The person's decision the setup tools respect: automation may narrow what agents may do, but widening
//! it (broader profile permissions or MCP servers, a CLI program of its own choice, environment variables
//! that change what programs load) is refused as `person_only` unless a person turned on *Let automation
//! change agent permissions* in the window. No tool changes that switch.

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  id::ResourceId,
  permission::{AgentPermissions, widening::Widening},
  resource::{AgentProfile, Pipeline},
  store::ResourceStore,
};
use orchester_store::ProjectDb;

use crate::{
  app_mcp::{error::ToolError, services::Services},
  messages::Message,
};

/// Refused unless nothing widens or a person lets automation widen; `reasons` say what would widen.
pub(in crate::app_mcp::tools) fn allowed(services: &Services, mut reasons: Vec<Message>) -> Result<(), ToolError> {
  if reasons.is_empty() || services.workspace.automation_changes_permissions()? {
    return Ok(());
  }
  reasons.insert(0, Message::new("appMcp.errors.personOnly"));
  Err(ToolError::refused("person_only", reasons))
}

/// Refused unless a person lets automation take what `profile` widens beyond the latest published version
/// of profile `id` (before its first version: the most restrictive permissions `provider` enforces here and
/// no MCP server). Approving automatically answers requests in a person's place, so it also needs *Let
/// automation answer approvals*, as auto mode does.
pub(super) fn check_profile(
  services: &Services,
  db: &ProjectDb,
  (id, profile): (Option<ResourceId>, &AgentProfile),
  provider: AgentProvider,
) -> Result<(), ToolError> {
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
  let widenings = profile.permissions().widenings_from(&base);
  let mut reasons: Vec<Message> = widenings.iter().copied().map(message).collect();
  if profile
    .mcp_servers()
    .ids()
    .iter()
    .any(|server| !servers.contains(server))
  {
    reasons.push(Message::new("appMcp.widens.mcpServers"));
  }
  allowed(services, reasons)?;
  if widenings.contains(&Widening::AutoApprove) && !services.workspace.automation_answers_approvals()? {
    return Err(ToolError::refused(
      "automation_not_allowed",
      vec![Message::new("appMcp.errors.autoApproveNeedsAnswers")],
    ));
  }
  Ok(())
}

/// Refused unless a person lets automation publish `new`, or it is provably no weaker than `latest`, the
/// pipeline's latest published version ([`weaker_than`]). A pipeline's first version has nothing to be
/// compared with, so it always needs a person. Every version automation publishes is thus one a person
/// published, allowed, or could not object to, so a task may run any published version.
pub(in crate::app_mcp::tools) fn check_pipeline(
  services: &Services,
  latest: Option<&Pipeline>,
  new: &Pipeline,
) -> Result<(), ToolError> {
  let reasons = crate::pipelines::widening::publish_reasons(latest, new);
  allowed(services, reasons)
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
