//! What the person lets the Orchestrator chat do itself, as its tools ask it ([`super::tools`],
//! [`super::mcp_servers`]): Autopilot of the project, the chat's switches, and whether automation may widen what agents
//! may do.

use orchester_store::ProjectDb;

use super::tools::ChatScope;
use crate::{
  app_mcp::{Services, ToolError, blocking},
  workspace::OrchestratorConsent,
};

/// Whether `scope`'s project allows Autopilot, which only a person turns on: the chat then starts runs itself.
pub(super) async fn allows_autopilot(scope: &ChatScope) -> Result<bool, ToolError> {
  let root = scope.root.clone();
  blocking(move || Ok(ProjectDb::open(&root)?.allow_autopilot()?.is_on())).await
}

/// Whether the person lets the Orchestrator chat do what `consent` names itself in `scope`'s project.
pub(super) async fn allows(
  services: &Services,
  scope: &ChatScope,
  consent: OrchestratorConsent,
) -> Result<bool, ToolError> {
  let (workspace, root) = (services.workspace().clone(), scope.root.clone());
  blocking(move || Ok(workspace.orchestrator_allows(&root, consent)?)).await
}

/// Whether the person lets automation widen what agents may do.
pub(super) async fn changes_permissions(services: &Services) -> Result<bool, ToolError> {
  let workspace = services.workspace().clone();
  blocking(move || Ok(workspace.automation_changes_permissions()?)).await
}
