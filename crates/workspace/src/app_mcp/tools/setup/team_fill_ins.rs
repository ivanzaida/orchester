//! What a computer fills in for a team project, over MCP (`docs/design/team-configuration.md`, "What a computer fills
//! in"): `set_reviewer_account` chooses what a team reviewer runs on here (a team profile's account is
//! `set_profile_account`'s, as a local profile's is), and the team MCP
//! servers `set_mcp_server_enabled` and `update_mcp_server` name are the team's resources, not the project's own
//! servers. Turning a team MCP server on lets agents run more here, which the person decided is theirs alone, in the
//! window (decision 12): over MCP it answers `person_only` always, which the switch *Let automation change agent
//! permissions* does not lift, before the write of [`crate::team_fill_ins`] is reached. Turning one off is taken. The
//! program's path on this computer is that window's too: no tool sets it. No tool takes or returns a secret value.

mod reviewer;

use std::{path::Path, sync::Arc};

use orchester_core::{resource::ResourceKind, run::Answerer};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::super::{Spec, parse};
use crate::{
  app_mcp::{error::ToolError, services::Services},
  messages::Message,
  team_config::{TeamConfig, TeamRead, TeamResource},
  team_fill_ins::FillInError,
};

pub(super) const SPECS: &[Spec] = &[reviewer::SPEC];

/// Calls tool `name` of [`SPECS`] with `arguments`.
pub(super) async fn call(services: &Services, name: &str, arguments: Value) -> Result<Value, ToolError> {
  match name {
    "set_reviewer_account" => reviewer::set_reviewer_account(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

/// Turns the team MCP server `server` off here, as `set_mcp_server_enabled` does; on is a person's alone, so it is
/// refused here, with nothing changed and without reaching the write that would take it from a person.
pub(super) fn switch_server(
  services: &Services,
  repo_root: &Path,
  server: &TeamResource,
  enabled: bool,
) -> Result<Value, ToolError> {
  if enabled {
    return Err(person_only("appMcp.errors.teamMcpOnPersonOnly"));
  }
  services
    .workspace
    .switch_team_mcp_on_here(repo_root, server.id, false, Answerer::Automation)
    .map_err(refusal)?;
  Ok(json!({ "server": { "id": server.id.to_string(), "name": server.name.as_str(), "team": true, "enabled": false } }))
}

/// The team's resource of `kind` that `named` names, by its id or its name, in the team's configuration of the project
/// at `repo_root` read this session; `None` for a name that is none of the team's. Refused `team_unreachable` (or
/// `team_reading`) while that configuration cannot be read.
pub(super) fn team_named(
  services: &Services,
  repo_root: &Path,
  kind: ResourceKind,
  named: &str,
) -> Result<Option<TeamResource>, ToolError> {
  let config = team_config(services, repo_root)?;
  let listed = config.resources(kind);
  // An id names one resource for certain; a name could also read as another's id.
  let found = listed
    .iter()
    .find(|resource| resource.id.to_string() == named)
    .or_else(|| listed.iter().find(|resource| resource.name.as_str() == named));
  Ok(found.map(|resource| (*resource).clone()))
}

/// The team's configuration of the project at `repo_root` read this session. Refused `team_unreachable` (or
/// `team_reading`) while none was read.
pub(super) fn team_config(services: &Services, repo_root: &Path) -> Result<Arc<TeamConfig>, ToolError> {
  match services.workspace.team_read(repo_root) {
    TeamRead::Read { config, .. } => Ok(config),
    TeamRead::Reading => Err(refusal(FillInError::TeamReading)),
    TeamRead::Unreachable(unreached) => Err(refusal(FillInError::TeamUnreachable(unreached))),
  }
}

pub(super) fn require_team_project(db: &ProjectDb) -> Result<(), ToolError> {
  if db.sharing()?.is_some() {
    Ok(())
  } else {
    Err(refusal(FillInError::NotTeamProject))
  }
}

fn person_only(reason: &'static str) -> ToolError {
  ToolError::refused(
    "person_only",
    vec![Message::new("appMcp.errors.personOnly"), Message::new(reason)],
  )
}

/// A fill-in the workspace refused, with the code a client matches on and the words the window shows.
pub(super) fn refusal(error: FillInError) -> ToolError {
  let code = match error {
    FillInError::Store(store) => return ToolError::from(store),
    FillInError::PersonOnly => return person_only("teamFillIns.errors.personOnly"),
    FillInError::NotTeamProject => "team_project_required",
    FillInError::NotTeamResource { .. } => "not_team_resource",
    FillInError::ConnectionNotFound(_) => "connection_not_found",
    FillInError::ConnectionDoesNotFit { .. } => "connection_does_not_fit",
    FillInError::DecisionConnectionNotFound(_) => "connection_not_found",
    FillInError::ReviewerIsLanguage(_) | FillInError::ReviewerIsDecision(_) => "connection_does_not_fit",
    FillInError::ServerNotAllowed(_) => "server_not_allowed",
    FillInError::ReviewerNotTeamForm(_) => "not_team_form",
    FillInError::AccountNotFound(_) => "account_not_found",
    FillInError::AccountOfOtherCli(_) => "account_of_other_cli",
    FillInError::ProgramNotAbsolute => "program_not_absolute",
    FillInError::ProgramNotAFile => "program_not_a_file",
    FillInError::RemoteServer(_) => "remote_server",
    FillInError::TeamReading => "team_reading",
    FillInError::TeamUnreachable(_) => "team_unreachable",
  };
  ToolError::refused(code, vec![error.message()])
}
