//! A start auto mode could not take because the task waits for the tasks that block it (`Ee2JE`): the refusal names
//! the task and those tasks, as they stood when the queue refused it.

use orchester_core::store::TaskStore;
use orchester_store::ProjectDb;

use super::tools::ChatScope;
use crate::{
  app_mcp::{ToolError, blocking},
  messages::Message,
};

/// What the queue says when unfinished blocking tasks keep a task from running.
const BLOCKED: &str = "tasks.blocked.runUnavailable";

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
