//! A tool that changes the team's configuration of a team project sends the change to the team server at once
//! (`docs/design/team-configuration-live.md`, rule 6): a new resource is created there with its first version, and its
//! answer is the tool's; nothing of it is kept on this computer before. While the server cannot be reached the change
//! is refused with why: there is nothing to save to and no outbox. A team's resource has no draft on this computer, so
//! a tool that saves or publishes a draft is refused in a team project ([`refuse_team_drafts`]).

use std::path::Path;

use orchester_core::id::ResourceId;
use rmcp::model::JsonObject;
use serde_json::Value;

use crate::{
  app_mcp::{error::ToolError, services::Services, tools::team},
  messages::Message,
  sharing::team_edit::{EditRefused, Local, NewTeamResource, Refusal},
};

/// Creates `new` on the team server of the project at `root`; its id.
pub(super) async fn create_team(
  services: &Services,
  root: &Path,
  new: NewTeamResource,
) -> Result<ResourceId, ToolError> {
  services
    .servers
    .create_team(root, new)
    .await
    .map_err(|error| refused(&error))
}

/// Why the team server did not take a change, as the tools refuse it.
pub(super) fn refused(error: &EditRefused) -> ToolError {
  let message = error.message();
  let code = match error {
    EditRefused::Refused(Refusal::NotAdmin | Refusal::NotAdminHere) => "not_admin",
    EditRefused::Refused(Refusal::Local(Local::Entities)) => "local_entities_named",
    EditRefused::Refused(Refusal::Local(Local::NamesNonTeam)) => "pipeline_names_non_team",
    EditRefused::Refused(Refusal::Local(Local::WritableFolders)) => "writable_folders_stay",
    EditRefused::Refused(Refusal::NameTaken) => "name_taken",
    EditRefused::Refused(_) => "team_server_refused",
    EditRefused::Changed { .. } => "version_conflict",
    EditRefused::Unreachable(_) => "team_unreachable",
    EditRefused::Failed(_) => "team_server_not_reached",
  };
  ToolError::refused(code, vec![message])
}

/// The tools that save or publish a draft of a profile, instruction, skill or tool. A team pipeline's tools
/// (`save_pipeline`, `create_pipeline`, `duplicate_pipeline`) send it to the team server at once and wait for its
/// answer instead (D-043), so they are not among them.
pub(crate) const DRAFT_TOOLS: [&str; 8] = [
  "save_profile_draft",
  "publish_profile",
  "save_instruction_draft",
  "publish_instruction",
  "save_tool_draft",
  "publish_tool",
  "save_skill_draft",
  "publish_skill",
];

/// A team's resource has no draft on this computer, and these tools make and publish drafts: in a team project they are
/// refused as `team_no_drafts` before anything is read or written. `arguments` name the project.
pub(crate) async fn refuse_team_drafts(services: &Services, arguments: &JsonObject) -> Result<(), ToolError> {
  let Some(project) = arguments.get("project").and_then(Value::as_str) else {
    return Ok(());
  };
  match team::of(services, project).await? {
    Some(_) => Err(ToolError::refused(
      "team_no_drafts",
      vec![Message::new("appMcp.errors.teamNoDrafts")],
    )),
    None => Ok(()),
  }
}
