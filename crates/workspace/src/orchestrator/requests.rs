//! The tool requests a conversation keeps as waits (`approval:<run>:<request>`), as its lines show them
//! (2026-11-11): which request each entry is, of which task, and, once it no longer waits, how it was answered and by
//! whom, so a request the person answered says so on its line.

use std::collections::HashMap;

use orchester_core::{
  id::{ChatId, RunId, TaskId},
  run::{DecidedBy, ToolApprovalDecision},
  store::RunStore,
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::detail::run_store_error;

/// A tool request a conversation keeps as a wait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRequest {
  /// The request's id.
  pub id: String,
  /// The task of its run.
  pub task: TaskId,
  /// How it was answered, once it no longer waits.
  pub decided: Option<RequestDecided>,
}

/// How a tool request that no longer waits went: its decision (`None` when its agent stopped waiting), who answered,
/// and whether the answer made a rule of the project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestDecided {
  pub decision: Option<ToolApprovalDecision>,
  pub by: Option<DecidedBy>,
  pub for_project: bool,
}

/// The tool requests `chat` keeps as waits, by entry.
pub(super) fn chat_requests(db: &ProjectDb, chat: ChatId) -> Result<HashMap<i64, ChatRequest>, StoreError> {
  let mut requests = HashMap::new();
  let kept: Vec<(i64, RunId, String)> = db
    .chat_wait_keys(chat)?
    .into_iter()
    .filter_map(|(seq, key)| request_of(&key).map(|(run, id)| (seq, run, id)))
    .collect();
  if kept.is_empty() {
    return Ok(requests);
  }
  let pending = db.pending_tool_approvals_all()?;
  for (seq, run, id) in kept {
    let Some(task) = db.run(run).map_err(run_store_error)?.map(|run| run.task()) else {
      continue;
    };
    let decided = if pending.iter().any(|request| request.id == id) {
      None
    } else {
      db.decided_tool_approvals(run)?
        .into_iter()
        .find(|decided| decided.request.id == id)
        .map(|decided| RequestDecided {
          decision: decided.decision,
          by: decided.decided_by,
          for_project: decided.for_project,
        })
    };
    requests.insert(seq, ChatRequest { id, task, decided });
  }
  Ok(requests)
}

/// The run and the request a wait's key names, for a tool request's wait.
fn request_of(key: &str) -> Option<(RunId, String)> {
  let rest = key.strip_prefix("approval:")?;
  let (run, id) = rest.split_once(':')?;
  Some((run.parse().ok()?, id.to_owned()))
}
