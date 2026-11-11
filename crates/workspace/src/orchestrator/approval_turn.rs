//! `decide_approval` in the chat while it drives (decided by the person on 2026-11-11, `docs/design/
//! orchestrator-toolset.md`, decision 14): a tool request of a run in what the conversation leads starts one turn by
//! itself, which approves that request once, denies it with a reason for the agent, or leaves it to the person. A turn
//! Orchester started decides only the request whose wait started it (`not_this_request`). The tool keeps its own
//! rules: `approve_once` or `deny`, never a standing rule, and only while a person lets automation answer approvals.

use orchester_core::{id::ChatId, orchestrator::WaitKind};
use orchester_store::{KeptWait, ProjectDb};
use rmcp::model::JsonObject;
use serde_json::Value;

use super::tools::ChatScope;
use crate::{
  app_mcp::{ToolError, blocking, find_run},
  messages::Message,
  tasks::recorded,
};

/// The app's tool that decides a tool request.
pub(super) const DECIDE_APPROVAL: &str = "decide_approval";

/// The key of the task whose tool request `arguments` name, while it waits; refused (`not_this_request`) in a turn
/// nobody asked for when the request is not the one whose wait started it.
pub(super) async fn checked(scope: &ChatScope, arguments: &JsonObject) -> Result<Option<String>, ToolError> {
  let Some(named) = arguments.get("approval").and_then(Value::as_str).map(str::to_owned) else {
    return Ok(None);
  };
  let (root, chat, on_its_own) = (scope.root.clone(), scope.chat, scope.on_its_own);
  blocking(move || {
    let db = ProjectDb::open(&root)?;
    if on_its_own && !started_by(&db.waits_thinking()?, chat, &named) {
      return Err(ToolError::refused(
        "not_this_request",
        vec![Message::new("orchestrator.tools.approvalNotThisRequest")],
      ));
    }
    let Some(request) = db
      .pending_tool_approvals_all()?
      .into_iter()
      .find(|request| request.id == named)
    else {
      return Ok(None);
    };
    let run = find_run(&db, &request.run.to_string())?;
    Ok(recorded::task_of(&db, run.task())?.map(|task| task.key().to_string()))
  })
  .await
}

/// Whether the turn thinking in `chat` among `thinking` is the one the wait of tool request `approval` started.
fn started_by(thinking: &[KeptWait], chat: ChatId, approval: &str) -> bool {
  let suffix = format!(":{approval}");
  thinking
    .iter()
    .any(|kept| kept.chat == chat && kept.wait.kind == WaitKind::Approval && kept.key.ends_with(&suffix))
}
