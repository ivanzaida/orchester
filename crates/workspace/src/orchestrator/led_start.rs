//! A start the chat does not take in a turn Orchester started by itself while it drives, because its conversation
//! does not lead the task (`docs/design/orchestrator-watch.md`, routing): such a turn acts only on the work its
//! conversation leads, so `run_task` there starts only a task whose events come to this conversation
//! ([`orchester_core::orchestrator::Subscriptions::leading`]). In a turn a person's message started, any task of the
//! project may be started.

use orchester_store::ProjectDb;

use super::tools::ChatScope;
use crate::{
  app_mcp::{ToolError, blocking, find_task},
  messages::Message,
};

/// Refuses (`not_led`) the chat's start of task `key` of `scope`'s project when another conversation, or none, leads
/// the task. A call that names no task is left to the start's own refusal.
pub(super) async fn led_here(scope: &ChatScope, key: Option<&str>) -> Result<(), ToolError> {
  let Some(named) = key.map(str::to_owned) else {
    return Ok(());
  };
  let (root, chat) = (scope.root.clone(), scope.chat);
  let (led, task) = blocking(move || {
    let db = ProjectDb::open(&root)?;
    let task = find_task(&db, &named)?;
    let leader = db.active_subscriptions()?.leading(task.id(), task.module());
    Ok((leader == Some(chat), task.key().to_string()))
  })
  .await?;
  if led {
    return Ok(());
  }
  Err(ToolError::refused(
    "not_led",
    vec![Message::new("orchestrator.start.notLed").with("task", task)],
  ))
}
