//! What the Orchestrator chat does not do itself (`docs/design/orchestrator-toolset.md`, decision 5), so that it never
//! takes up its own doing in a loop and never decides what is the person's: in a turn a wait or a run's end started by
//! itself ([`super::tools::ChatScope::on_its_own`]) nobody asked for anything, so a merge, a start, a pause, a resume,
//! a release of new starts and a publication are only proposed, whatever the person's switches and auto mode; a
//! message that moves a run none of whose agents waits or works (a follow-up, a continuation, a resume, the task run
//! again) is always proposed; and a run stopped at a limit or interrupted is resumed by the person only.

use orchester_core::{id::RunId, orchestrator::change::Publishing};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::Value;

use super::{
  change::control::{self, PAUSE_RUN, RELEASE_QUEUE, RESUME_RUN},
  tools::{ChatScope, MERGE_RUN},
};
use crate::app_mcp::{ToolError, blocking, find_run};

/// Whether the chat's call of `name` in `scope` is only proposed as a change because nobody asked for anything in its
/// turn; a start is then proposed as a start ([`super::tools`]).
pub(super) fn proposed_on_its_own(scope: &ChatScope, name: &str) -> bool {
  scope.on_its_own
    && (matches!(name, MERGE_RUN | PAUSE_RUN | RESUME_RUN | RELEASE_QUEUE) || Publishing::of_tool(name).is_some())
}

/// The run `resume_run` with `arguments` resumes in `scope`, when the chat may resume it: one a pause stopped. One
/// stopped at a limit or interrupted is the person's (`person_only`).
pub(super) async fn resumable(scope: &ChatScope, arguments: &JsonObject) -> Result<RunId, ToolError> {
  let named = arguments
    .get("run")
    .and_then(Value::as_str)
    .ok_or_else(|| ToolError::InvalidArguments("run: required".to_owned()))?
    .to_owned();
  let root = scope.root.clone();
  let run = blocking(move || find_run(&ProjectDb::open(&root)?, &named)).await?;
  control::resumable(&run, control::ResumeRefusal::Agent)?;
  Ok(run.id())
}
