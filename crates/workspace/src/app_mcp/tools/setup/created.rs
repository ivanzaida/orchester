//! A tool that makes a new profile, instruction, skill or tool creates it as v1 in one call, with no draft: it returns
//! the published resource or a refusal. In a team project the version is the team's only once the team server accepts
//! it ([`settle`]); offline it waits in the outbox and is reported as such.

use std::path::PathBuf;

use orchester_core::id::ResourceId;
use serde_json::{Value, json};

use crate::{
  app_mcp::{error::ToolError, services::Services},
  messages::Message,
  sharing::{Created, NotCreated},
};

/// Settles resource `id`, just created as v1 in the project at `root`, with the team server (nothing to do in a local
/// project): taken back, and refused as `team_server_refused`, `local_entities_named` (its body names something of
/// this computer, until ORC-820 gives team profiles their own body), `pipeline_names_non_team` (a pipeline runs a
/// profile or tool the team does not have) or `team_server_not_reached`, when it was not taken. Otherwise `Some` says
/// it waits for the server, to be added to the answer.
pub(super) async fn settle(services: &Services, root: PathBuf, id: ResourceId) -> Result<Option<Value>, ToolError> {
  match services.servers.send_created(&root, id).await {
    Ok(Created::Accepted) => Ok(None),
    Ok(Created::Waiting) => Ok(Some(json!({ "code": "team_server_unreachable" }))),
    Err(NotCreated::Refused { code }) => Err(ToolError::refused(
      "team_server_refused",
      vec![Message::new("appMcp.errors.teamServerRefused").with("code", code)],
    )),
    Err(NotCreated::NamesLocal(reason)) => Err(ToolError::refused("local_entities_named", vec![reason])),
    Err(NotCreated::NamesNonTeam(reason)) => Err(ToolError::refused("pipeline_names_non_team", vec![reason])),
    Err(NotCreated::Failed(reason)) => Err(ToolError::refused("team_server_not_reached", vec![reason])),
  }
}

/// `answer`'s `key` resource, with `waiting` (from [`settle`]) as `waiting_for_server` when it waits for the server.
pub(super) fn with_waiting(mut answer: Value, key: &str, waiting: Option<Value>) -> Value {
  if let Some(waiting) = waiting {
    answer[key]["waiting_for_server"] = waiting;
  }
  answer
}
