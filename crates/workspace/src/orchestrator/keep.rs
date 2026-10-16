//! What the Orchestrator's agent changed, or tried to, kept in its conversation as an action: the tool, the task it
//! names as people read it, how it went, and for a hold of new starts what it held.

use orchester_core::{
  id::RunId,
  orchestrator::{ActionOutcome, ChatAction},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::Value;

use super::tools::ChatScope;
use crate::{
  app_mcp::{ToolError, Translate},
  tasks::hold,
};

/// Which task a call names: its key, or the run it names, read when the action is kept.
pub(super) enum Named {
  Task(String),
  Run(String),
}

pub(super) fn named_task(arguments: &JsonObject) -> Option<Named> {
  let text = |name: &str| arguments.get(name).and_then(Value::as_str).map(str::to_owned);
  text("key")
    .or_else(|| text("task"))
    .map(Named::Task)
    .or_else(|| text("run").map(Named::Run))
}

/// The key of the task a created task's result names.
pub(super) fn created_task(result: &Value) -> Option<Named> {
  result
    .pointer("/task/key")
    .and_then(Value::as_str)
    .map(|key| Named::Task(key.to_owned()))
}

/// The refusal `error` as the conversation keeps it, in the words of `translate`.
pub(super) fn refusal(error: &ToolError, translate: &Translate) -> ActionOutcome {
  let refused = error.to_json(|message| translate.text(message));
  let text = |name: &str| {
    refused
      .pointer(&format!("/error/{name}"))
      .and_then(Value::as_str)
      .unwrap_or_default()
      .to_owned()
  };
  ActionOutcome::Refused {
    code: text("code"),
    reason: text("message"),
  }
}

/// The tool whose action keeps what it held.
pub(super) const HOLD_QUEUE: &str = "hold_queue";

/// The task a hold named, which names its group; `None` for the whole project.
pub(super) fn arguments_task(named: &Option<Named>) -> Option<String> {
  match named {
    Some(Named::Task(task)) => Some(task.clone()),
    Some(Named::Run(_)) | None => None,
  }
}

/// Keeps `action` in the conversation, with the key of the task `named` names and, for a hold of new starts (`held`:
/// the task that names its group, if any), what it held; one that cannot be kept is logged.
pub(super) async fn keep(
  scope: &ChatScope,
  (named, held): (Option<Named>, Option<Option<String>>),
  mut action: ChatAction,
) {
  let (root, chat) = (scope.root.clone(), scope.chat);
  let kept = tokio::task::spawn_blocking(move || {
    let mut db = ProjectDb::open(&root)?;
    action.task = named.and_then(|named| task_key(&db, named));
    action.held = match held {
      Some(named) => {
        let group = match named {
          Some(named) => hold::group_of_named(&db, &named)?,
          None => None,
        };
        Some(hold::held_work(&db, group)?)
      }
      None => None,
    };
    db.add_chat_action(chat, &action, Timestamp::now())
  })
  .await;
  match kept {
    Ok(Ok(())) => {}
    Ok(Err(error)) => tracing::error!(%chat, %error, "cannot keep an action of the Orchestrator"),
    Err(error) => tracing::error!(%chat, %error, "keeping an action of the Orchestrator did not finish"),
  }
}

/// The key of the task `named` names, as people read it; `None` when it cannot be found.
fn task_key(db: &ProjectDb, named: Named) -> Option<String> {
  match named {
    Named::Task(key) => Some(key),
    Named::Run(run) => {
      let run = db.run(run.parse::<RunId>().ok()?).ok()??;
      Some(db.task(run.task()).ok()??.key().to_string())
    }
  }
}
