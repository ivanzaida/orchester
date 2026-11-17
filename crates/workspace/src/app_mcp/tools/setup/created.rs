//! A tool that changes the team's configuration of a team project sends the change to the team server at once
//! (`docs/design/team-configuration-live.md`, rule 6): a new resource is created there with its first version, and its
//! answer is the tool's; nothing of it is kept on this computer before. While the server cannot be reached the change
//! is refused with why: there is nothing to save to and no outbox. A team's resource has no draft on this computer, so
//! a tool that saves a draft of one publishes its next version there at once instead ([`super::team_resources`]).

use std::path::Path;

use orchester_core::id::ResourceId;

use crate::{
  app_mcp::{error::ToolError, services::Services},
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
