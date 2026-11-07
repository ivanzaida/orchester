//! `run_task`'s answer when the team server built no bundle for the start (`bundle_missing`): the reason it gives, in a
//! message per kind that says what to do. The resource a reason names is shown by the name this computer has cached
//! for it, or by its id when it has none.

use orchester_api::v2::{BundleMissingKind, BundleMissingReason, ErrorDetails};
use orchester_core::{id::ResourceId, store::ResourceStore};
use orchester_store::ProjectDb;

use super::Team;
use crate::{
  app_mcp::{error::ToolError, services::blocking},
  messages::Message,
};

const CODE: &str = "team_bundle_missing";

/// The refusal for a start the server answered `bundle_missing` with `details`: nothing was started.
pub(super) async fn refused(team: &Team, details: &ErrorDetails) -> ToolError {
  let reason = match details {
    ErrorDetails::BundleMissing { reason } => Some(reason),
    _ => None,
  };
  match message(team, reason).await {
    Ok(message) => ToolError::refused(CODE, vec![message]),
    Err(error) => error,
  }
}

async fn message(team: &Team, reason: Option<&BundleMissingReason>) -> Result<Message, ToolError> {
  let Some(reason) = reason else {
    return Ok(Message::new("appMcp.errors.teamBundleMissing"));
  };
  let (key, argument) = match reason.kind {
    BundleMissingKind::PipelineMissing => ("appMcp.errors.teamBundleMissingPipeline", "pipeline"),
    BundleMissingKind::CliKindMissing => ("appMcp.errors.teamBundleMissingCliKind", "profile"),
    BundleMissingKind::McpServersUnresolved => ("appMcp.errors.teamBundleMissingMcpServers", "profile"),
    BundleMissingKind::McpServerArchived => ("appMcp.errors.teamBundleMissingMcpArchived", "server"),
    BundleMissingKind::FileMissing => return Ok(Message::new("appMcp.errors.teamBundleMissingFile")),
    BundleMissingKind::BodyUnreadable => return Ok(Message::new("appMcp.errors.teamBundleMissingBody")),
    BundleMissingKind::BundleTooLarge => return Ok(Message::new("appMcp.errors.teamBundleMissingTooLarge")),
    BundleMissingKind::Other => return Ok(Message::new("appMcp.errors.teamBundleMissing")),
  };
  // These kinds are about one resource; an answer that names none says no more than `other` does.
  let Some(resource) = reason.resource_id else {
    return Ok(Message::new("appMcp.errors.teamBundleMissing"));
  };
  let name = resource_name(team, resource).await?;
  Ok(Message::new(key).with(argument, name))
}

/// The name this computer has cached for `resource`, or its id.
async fn resource_name(team: &Team, resource: ResourceId) -> Result<String, ToolError> {
  let root = team.open.repo_root.clone();
  blocking(move || {
    let cached = ProjectDb::open(&root)?.resource(resource)?;
    Ok(cached.map_or_else(|| resource.to_string(), |cached| cached.name().as_str().to_owned()))
  })
  .await
}
