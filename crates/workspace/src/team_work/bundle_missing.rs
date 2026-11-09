//! What a start the team server refused `bundle_missing` says to the person: a message per kind of reason that says
//! what to do, as `run_task` answers it and the task page shows it. The resource a reason names is shown by the name
//! this computer has cached for it, or by its id when it has none.

use std::path::Path;

use orchester_api::v2::{BundleMissingKind, BundleMissingReason};
use orchester_core::{id::ResourceId, store::ResourceStore};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::Message;

/// The message for the refusal's `reason` (`None` when the server did not say why), reading the cached name of the
/// resource it names from the project at `root`. Blocks on SQLite.
pub fn bundle_missing_message(root: &Path, reason: Option<&BundleMissingReason>) -> Result<Message, StoreError> {
  let Some(reason) = reason else {
    return Ok(Message::new("appMcp.errors.teamBundleMissing"));
  };
  let (key, argument) = match reason.kind {
    BundleMissingKind::PipelineMissing => ("appMcp.errors.teamBundleMissingPipeline", "pipeline"),
    BundleMissingKind::ProfileUnreadable => ("appMcp.errors.teamBundleMissingProfile", "profile"),
    BundleMissingKind::McpServerArchived => ("appMcp.errors.teamBundleMissingMcpArchived", "server"),
    BundleMissingKind::FileMissing => ("appMcp.errors.teamBundleMissingFile", "resource"),
    BundleMissingKind::BodyUnreadable => ("appMcp.errors.teamBundleMissingBody", "resource"),
    BundleMissingKind::BundleTooLarge => return Ok(Message::new("appMcp.errors.teamBundleMissingTooLarge")),
    BundleMissingKind::Other => return Ok(Message::new("appMcp.errors.teamBundleMissing")),
  };
  // These kinds are about one resource; an answer that names none says no more than `other` does.
  let Some(resource) = reason.resource_id else {
    return Ok(Message::new("appMcp.errors.teamBundleMissing"));
  };
  Ok(Message::new(key).with(argument, resource_name(root, resource)?))
}

/// The name this computer has cached for `resource`, or its id.
fn resource_name(root: &Path, resource: ResourceId) -> Result<String, StoreError> {
  let cached = ProjectDb::open(root)?.resource(resource)?;
  Ok(cached.map_or_else(|| resource.to_string(), |cached| cached.name().as_str().to_owned()))
}
