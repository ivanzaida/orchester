//! The person's decision the setup tools respect: automation may narrow what agents may do, but widening
//! it (broader profile permissions or MCP servers, a CLI program of its own choice, environment variables
//! that change what programs load) is refused as `person_only` unless a person turned on *Let automation
//! change agent permissions* in the window, or the person decided the call themselves (they applied a proposal of
//! the Orchestrator chat, `Services::decided_by_person`): then exactly what they were shown widens is taken, and
//! anything else is refused. No tool changes that switch.

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  permission::widening::Widening,
  resource::{AgentProfile, Pipeline},
};
use orchester_store::ProjectDb;

use crate::{
  app_mcp::{error::ToolError, services::Services},
  messages::Message,
};

/// Refused unless nothing widens or a person lets automation widen; `reasons` say what would widen.
pub(in crate::app_mcp::tools) fn allowed(services: &Services, mut reasons: Vec<Message>) -> Result<(), ToolError> {
  if let Some(decided) = services.widening_decided(&reasons) {
    return decided;
  }
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
  let reasons = crate::agents::widening::reasons(db, id, profile, provider)?;
  let auto = reasons.contains(&crate::agents::widening::message(Widening::AutoApprove));
  allowed(services, reasons)?;
  if auto && services.person.is_none() && !services.workspace.automation_answers_approvals()? {
    return Err(ToolError::refused(
      "automation_not_allowed",
      vec![Message::new("appMcp.errors.autoApproveNeedsAnswers")],
    ));
  }
  Ok(())
}

/// Refused unless a person lets automation publish `new`, or it is provably no weaker than `latest`, the
/// pipeline's latest published version (by their publishing policy). A pipeline's first version has nothing to be
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
