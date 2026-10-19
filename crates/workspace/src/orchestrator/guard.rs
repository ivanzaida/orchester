//! What the Orchestrator chat does not do itself (`docs/design/orchestrator-toolset.md`, decision 5), so that it never
//! takes up its own doing in a loop and never decides what is the person's: in a turn a wait or a run's end started by
//! itself, nobody asked for anything, so a merge and a start are only proposed, whatever the person's switches and
//! auto mode; and a run stopped at a limit (its budget or time, the daily or a group's budget, a step's budget, an
//! account's plan) or interrupted is resumed by the person only.

use orchester_core::{
  id::RunId,
  run::{PauseReason, RunStatus},
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::Value;

use super::tools::ChatScope;
use crate::{
  app_mcp::{ToolError, blocking, find_run},
  messages::Message,
};

/// Whether the turn under way in `scope`'s conversation is one a wait or a run's end started by itself.
pub(super) async fn on_its_own(scope: &ChatScope) -> Result<bool, ToolError> {
  let (root, chat) = (scope.root.clone(), scope.chat);
  blocking(move || Ok(ProjectDb::open(&root)?.thinks_on_its_own(chat)?)).await
}

/// The run `resume_run` with `arguments` resumes in `scope`, when the chat may resume it: one a pause asked for
/// stopped. One stopped at a limit or interrupted is the person's (`person_only`).
pub(super) async fn resumable(scope: &ChatScope, arguments: &JsonObject) -> Result<RunId, ToolError> {
  let named = arguments
    .get("run")
    .and_then(Value::as_str)
    .ok_or_else(|| ToolError::InvalidArguments("run: required".to_owned()))?
    .to_owned();
  let root = scope.root.clone();
  let run = blocking(move || find_run(&ProjectDb::open(&root)?, &named)).await?;
  match run.status() {
    RunStatus::Paused(PauseReason::LimitReached(_)) | RunStatus::Interrupted(_) => Err(ToolError::refused(
      "person_only",
      vec![Message::new("orchestrator.tools.resumePersonOnly").with("run", run.number().get().to_string())],
    )),
    _ => Ok(run.id()),
  }
}
