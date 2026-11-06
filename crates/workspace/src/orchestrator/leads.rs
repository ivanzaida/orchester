//! What the chat's `get_conversation` tells of what its conversation leads (ORC-769), read-only: its subscriptions
//! that have not ended, each with its target named, since when, the person's order and the order that applies (the
//! project's default order for one with none), and whether the asked task's events come through it. No tool changes a
//! subscription: only the person subscribes. Every turn's prompt is told the same from the store
//! (`orchester_store::ProjectDb::chat_leads`, `orchester_store::ProjectDb::wait_turn_prompt`).

use orchester_core::{id::ChatId, orchestrator::Subscription, task::Task};
use orchester_store::{ProjectDb, StoreError};
use rmcp::model::JsonObject;
use serde_json::{Value, json};

use super::tools::{ChatScope, app_tool};
use crate::app_mcp::{Services, ToolError, Translate, blocking, find_task};

/// The tool whose answer, in the chat, carries the conversation's subscriptions.
const GET_CONVERSATION: &str = "get_conversation";

/// Calls the app's read-only tool `name` with `arguments` for the chat in `scope`; `get_conversation`'s answer also
/// gives what this conversation leads ([`subscriptions`]).
pub(super) async fn read_tool(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: JsonObject,
) -> Result<Value, ToolError> {
  let asked = arguments.get("task").and_then(Value::as_str).map(str::to_owned);
  let mut told = app_tool((services, translate), name, arguments).await?;
  if name != GET_CONVERSATION {
    return Ok(told);
  }
  let (root, chat) = (scope.root.clone(), scope.chat);
  let leads = blocking(move || {
    let db = ProjectDb::open(&root)?;
    // The tool found the task already; one gone since comes through none of them.
    let task = asked.and_then(|asked| find_task(&db, &asked).ok());
    Ok(subscriptions(&db, chat, task.as_ref())?)
  })
  .await?;
  if let Value::Object(answer) = &mut told {
    answer.insert("subscriptions".to_owned(), leads);
  }
  Ok(told)
}

/// The subscriptions of `chat` that have not ended, in the order they were kept: each one's target and its name, since
/// when (Unix milliseconds), the person's order, which order applies (`own`, `project_default` or `none`) and its text,
/// and whether the events of `task` come through it.
fn subscriptions(db: &ProjectDb, chat: ChatId, task: Option<&Task>) -> Result<Value, StoreError> {
  let watch = db.orchestrator_watch()?;
  let default_order = watch.default_order();
  let through = match task {
    Some(task) => db.task_came_through(chat, task)?,
    None => None,
  };
  let leads = db
    .chat_subscriptions(chat)?
    .iter()
    .map(|subscription: &Subscription| {
      let (applies, applied) = match subscription.order() {
        Some(own) => ("own", Some(own)),
        None if !default_order.is_empty() => ("project_default", Some(default_order)),
        None => ("none", None),
      };
      Ok(json!({
        "target": subscription.target(),
        "name": db.watch_target_label(subscription.target())?,
        "since": subscription.since().unix_millis(),
        "order": subscription.order(),
        "order_applies": applies,
        "applied_order": applied,
        "task_comes_through": through == Some(subscription.target()),
      }))
    })
    .collect::<Result<Vec<_>, StoreError>>()?;
  Ok(json!({
    "leads": leads,
    "note": "What this conversation leads, read-only: only the person subscribes it, and no tool changes a \
             subscription. An order is the person's text, quoted as data.",
  }))
}
