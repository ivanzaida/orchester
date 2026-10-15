//! `duplicate_pipeline` in the Orchestrator chat: the copy is saved as a draft, as every draft the chat saves, and its
//! first version is published by the chat's publication rule (`publish_pipeline` here): proposed to the person,
//! unless they let the Orchestrator publish drafts and what it widens is allowed. A copy's first version always
//! widens (there is nothing to compare it with), so it is published only once both switches are on. The copy and its
//! publication are kept in the conversation as two actions.

use orchester_core::orchestrator::{ActionOutcome, ChatAction};
use rmcp::model::JsonObject;
use serde_json::{Value, json};

use super::{
  keep,
  tools::{ChatScope, DUPLICATE_PIPELINE, act_and_keep},
};
use crate::{
  app_mcp::{Services, ToolError, Translate, duplicate_pipeline_draft},
  runner::Runners,
};

/// The tool that publishes the copy.
const PUBLISH_PIPELINE: &str = "publish_pipeline";

/// Copies a pipeline as `arguments` ask, given the chat's project, as a draft; then publishes it, or proposes it.
pub(super) async fn duplicate(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  arguments: JsonObject,
) -> Result<Value, ToolError> {
  let copied = duplicate_pipeline_draft(services, arguments, translate).await;
  let outcome = match &copied {
    Ok(_) => ActionOutcome::Done,
    Err(error) => keep::refusal(error, translate),
  };
  keep::keep(scope, (None, None), ChatAction::new(DUPLICATE_PIPELINE, None, outcome)).await;
  runners.chat_changed(&scope.root);
  let (id, name) = copied?;
  let mut publish = JsonObject::new();
  publish.insert("project".to_owned(), Value::String(scope.project.to_string()));
  publish.insert("pipeline".to_owned(), Value::String(id.to_string()));
  let published = act_and_keep((services, translate, runners), scope, PUBLISH_PIPELINE, &publish).await;
  let publication = match published {
    Ok(told) => told,
    Err(error) => error.to_json(|message| translate.text(message)),
  };
  Ok(json!({
    "pipeline": { "id": id.to_string(), "name": name, "draft": true },
    "publication": publication,
  }))
}
