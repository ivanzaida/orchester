//! `duplicate_pipeline` in the Orchestrator chat while it drives (D-033; refused otherwise,
//! [`super::guard::pipeline_refused`]): the copy is published as its first version at once, as for any client (a
//! pipeline is always a published version, D-032): a copy that widens what agents may do is refused (`person_only`)
//! unless a person turned on *Let automation change agent permissions*. The copy is kept in the conversation as an
//! action.

use orchester_core::orchestrator::{ActionOutcome, ChatAction};
use rmcp::model::JsonObject;
use serde_json::Value;

use super::{
  keep,
  tools::{ChatScope, DUPLICATE_PIPELINE},
};
use crate::{
  app_mcp::{Services, ToolError, Translate, duplicate_pipeline_in_chat},
  runner::Runners,
};

/// Copies a pipeline as `arguments` ask, given the chat's project, and keeps what happened.
pub(super) async fn duplicate(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  arguments: JsonObject,
) -> Result<Value, ToolError> {
  let copied = duplicate_pipeline_in_chat(services, arguments, translate).await;
  let outcome = match &copied {
    Ok(_) => ActionOutcome::Done,
    Err(error) => keep::refusal(error, translate),
  };
  keep::keep(scope, (None, None), ChatAction::new(DUPLICATE_PIPELINE, None, outcome)).await;
  runners.chat_changed(&scope.root);
  copied
}
