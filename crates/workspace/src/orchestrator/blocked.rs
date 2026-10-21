//! A start the chat does not take because the task is blocked. A task a person or the chat marked Blocked is never
//! started from the chat, proposed or not (ORC-58): the manual Blocked keeps no run from starting, so the person still
//! starts it in the app. A start auto mode could not take because the task waits for the tasks that block it
//! (`Ee2JE`): the refusal names the task and those tasks, as they stood when the queue refused it.

use orchester_core::store::TaskStore;
use orchester_store::ProjectDb;

use super::tools::ChatScope;
use crate::{
  app_mcp::{ToolError, blocking, find_task},
  messages::Message,
};

/// What the queue says when unfinished blocking tasks keep a task from running.
const BLOCKED: &str = "tasks.blocked.runUnavailable";

/// Refuses (`task_blocked`) the chat's start of task `key` of `scope`'s project when it is marked Blocked, naming its
/// reason. A call that names no task is left to the start's own refusal.
pub(super) async fn marked(scope: &ChatScope, key: Option<&str>) -> Result<(), ToolError> {
  let Some(named) = key.map(str::to_owned) else {
    return Ok(());
  };
  let root = scope.root.clone();
  let task = blocking(move || find_task(&ProjectDb::open(&root)?, &named)).await?;
  match task.organization().blocked_reason() {
    Some(reason) => Err(ToolError::refused(
      "task_blocked",
      vec![
        Message::new("orchestrator.start.markedBlocked")
          .with("task", task.key().to_string())
          .with("reason", reason.to_owned()),
      ],
    )),
    None => Ok(()),
  }
}

/// `error`, naming task `key` and the tasks that block it when it is the queue's refusal of a blocked task. The
/// queue's own refusal stays when they cannot be read.
pub(super) async fn named(scope: &ChatScope, key: Option<&str>, error: ToolError) -> ToolError {
  let (ToolError::Refused { code, reasons }, Some(key)) = (&error, key) else {
    return error;
  };
  if !reasons.iter().any(|reason| reason.key == BLOCKED) {
    return error;
  }
  let (root, named) = (scope.root.clone(), key.to_owned());
  match blocking(move || blockers(&ProjectDb::open(&root)?, &named)).await {
    Ok(Some((task, blockers))) => ToolError::refused(
      code.clone(),
      vec![
        Message::new("orchestrator.start.blocked")
          .with("task", task)
          .with("blockers", blockers.join(", ")),
      ],
    ),
    Ok(None) => error,
    Err(read) => {
      tracing::warn!(?read, "cannot read the tasks that block a start of the Orchestrator");
      error
    }
  }
}

/// The key of task `named` and the keys of the unfinished tasks that block it; `None` without either.
fn blockers(db: &ProjectDb, named: &str) -> Result<Option<(String, Vec<String>)>, ToolError> {
  let lowered = named.to_lowercase();
  let Some(task) = db
    .tasks()?
    .into_iter()
    .find(|task| task.key().to_string().to_lowercase() == lowered || task.id().to_string() == named)
  else {
    return Ok(None);
  };
  let mut keys = Vec::new();
  for blocker in db.unfinished_blocking_tasks(task.id())? {
    if let Some(blocker) = db.task(blocker)? {
      keys.push(blocker.key().to_string());
    }
  }
  Ok((!keys.is_empty()).then(|| (task.key().to_string(), keys)))
}
