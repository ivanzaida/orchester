//! `decide_approval` in the chat while it drives (decided by the person on 2026-11-11, `docs/design/
//! orchestrator-toolset.md`, decision 14): a tool request of a run in what the conversation leads starts one turn by
//! itself, which approves that request once, denies it with a reason for the agent, or leaves it to the person. A turn
//! Orchester started decides only the request whose wait started it (`not_this_request`). The tool keeps its own
//! rules (`approve_once` or `deny`, never a standing rule); the drive switch is what lets the chat answer. The chat
//! gives its reason (`reason`, the chat's own argument), kept with what was asked as its answer ([`ToolAnswer`]).

use std::sync::Arc;

use orchester_core::{
  id::ChatId,
  orchestrator::{ToolAnswer, WaitKind},
  run::PendingToolApproval,
};
use orchester_store::{KeptWait, ProjectDb};
use rmcp::model::{JsonObject, Tool};
use serde_json::{Value, json};

use super::tools::ChatScope;
use crate::{
  app_mcp::{ToolError, blocking, find_run},
  messages::Message,
  tasks::{
    approvals::{self, hold_cause},
    recorded,
  },
};

/// The app's tool that decides a tool request.
pub(super) const DECIDE_APPROVAL: &str = "decide_approval";

/// The decision that allows a request once.
const APPROVE_ONCE: &str = "approve_once";

/// The chat's own argument of [`DECIDE_APPROVAL`]: its reason, which the conversation shows under the request.
const REASON: &str = "reason";

/// `tool`, given the chat's `reason` when it is [`DECIDE_APPROVAL`].
pub(super) fn with_reason(tool: &mut Tool) {
  if tool.name != DECIDE_APPROVAL {
    return;
  }
  let mut schema = (*tool.input_schema).clone();
  if let Some(Value::Object(properties)) = schema.get_mut("properties") {
    properties.insert(
      REASON.to_owned(),
      json!({
        "type": "string",
        "description": "Why you allow or deny it, in one sentence; the conversation shows it under the request.",
      }),
    );
  }
  if let Some(Value::Array(required)) = schema.get_mut("required") {
    required.push(Value::from(REASON));
  }
  tool.input_schema = Arc::new(schema);
}

/// The chat's reason, and `arguments` without it, as the app's tool takes them.
pub(super) fn reason(arguments: &JsonObject) -> Result<(String, JsonObject), ToolError> {
  let mut rest = arguments.clone();
  let reason = rest
    .remove(REASON)
    .and_then(|reason| reason.as_str().map(str::trim).map(str::to_owned))
    .filter(|reason| !reason.is_empty())
    .ok_or_else(|| ToolError::InvalidArguments(format!("{REASON}: required")))?;
  Ok((reason, rest))
}

/// What a decision's arguments name: the task of the request, and the request while it waits.
pub(super) struct Checked {
  pub(super) task: Option<String>,
  request: Option<PendingToolApproval>,
}

impl Checked {
  /// The chat's answer, from the app tool's answer `told`, with `reason`; `None` for a request it did not read.
  pub(super) fn answer(&self, told: &Value, reason: String) -> Option<ToolAnswer> {
    let request = self.request.as_ref()?;
    let decided = told.pointer("/approval/decided").and_then(Value::as_str)?;
    Some(ToolAnswer {
      request: request.id.clone(),
      asked: asked(request),
      allowed: decided != "deny",
      reason,
      told: told
        .pointer("/approval/message")
        .and_then(Value::as_str)
        .map(str::to_owned),
    })
  }
}

/// What `request` asks, as the conversation shows it: a command, or an MCP server's tool.
fn asked(request: &PendingToolApproval) -> String {
  if request.command {
    request.action.clone()
  } else {
    format!("{} · {}", request.server, request.action)
  }
}

/// The task and the request `arguments` name, while it waits; refused (`not_this_request`) in a turn nobody asked for
/// when the request is not the one whose wait started it, (`reviewer_judging`) while auto mode's reviewer judges it,
/// and (`hard_hold`) an approval of one a hard rule of auto mode holds ([`approvals::hard_hold`]: a push, a destructive
/// change, a secret, a write outside the folders, …): the chat may only deny such a request or leave it to the person,
/// in any turn, whatever its agent wrote.
pub(super) async fn checked(scope: &ChatScope, arguments: &JsonObject) -> Result<Checked, ToolError> {
  let Some(named) = arguments.get("approval").and_then(Value::as_str).map(str::to_owned) else {
    return Ok(Checked {
      task: None,
      request: None,
    });
  };
  let (root, chat, on_its_own) = (scope.root.clone(), scope.chat, scope.on_its_own);
  let approves = arguments.get("decision").and_then(Value::as_str) == Some(APPROVE_ONCE);
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
      return Ok(Checked {
        task: None,
        request: None,
      });
    };
    if db.reviewing_tool_approvals()?.contains(&request.id) {
      return Err(ToolError::refused(
        "reviewer_judging",
        vec![Message::new("orchestrator.tools.approvalReviewerJudging")],
      ));
    }
    if let Some(hold) = approves.then(|| approvals::hard_hold(&db, &root, &request)).flatten() {
      return Err(ToolError::refused(
        "hard_hold",
        vec![Message::new("orchestrator.tools.approvalHardHold"), hold_cause(hold)],
      ));
    }
    let run = find_run(&db, &request.run.to_string())?;
    let task = recorded::task_of(&db, run.task())?.map(|task| task.key().to_string());
    Ok(Checked {
      task,
      request: Some(request),
    })
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
