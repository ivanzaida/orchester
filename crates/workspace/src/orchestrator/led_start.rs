//! What a turn Orchester started by itself may start while the chat drives (`docs/design/orchestrator-watch.md`,
//! routing; `docs/design/orchestrator-toolset.md`, decision 14): only in the turn of a run's success it merged, where
//! an order of the conversation applies (`not_merge_turn` in any other), only a task its conversation leads
//! (`not_led`, [`orchester_core::orchestrator::Subscriptions::leading`]), and only a task that has no run yet
//! (`already_ran`): running a task again is the person's, so a failure's turn can never start its task once more. In a
//! turn a person's message started, any task of the project may be started.

use orchester_core::{
  id::ChatId,
  orchestrator::{ActionOutcome, ChatEntry, EntryKind, Merging, RunEnd, WaitKind},
  store::RunStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::tools::{ChatScope, MERGE_RUN};
use crate::{
  app_mcp::{ToolError, blocking, find_task},
  messages::Message,
  tasks::detail::run_store_error,
};

/// Refuses the chat's start of task `key` of `scope`'s project in a turn nobody asked for: outside the turn of a
/// success it merged under an order (`not_merge_turn`), of a task another conversation, or none, leads (`not_led`), or
/// of a task that has a run already (`already_ran`). A call that names no task is left to the start's own refusal.
pub(super) async fn led_here(scope: &ChatScope, key: Option<&str>) -> Result<(), ToolError> {
  let Some(named) = key.map(str::to_owned) else {
    return Ok(());
  };
  let (root, chat) = (scope.root.clone(), scope.chat);
  let refusal = blocking(move || {
    let db = ProjectDb::open(&root)?;
    let task = find_task(&db, &named)?;
    let key = task.key().to_string();
    if !merged_under_order(&db, chat, &db.chat_entries(chat)?)? {
      return Ok(Some((
        "not_merge_turn",
        Message::new("orchestrator.start.notMergeTurn"),
      )));
    }
    if db.active_subscriptions()?.leading(task.id(), task.module()) != Some(chat) {
      return Ok(Some((
        "not_led",
        Message::new("orchestrator.start.notLed").with("task", key),
      )));
    }
    if !db.runs_of_task(task.id()).map_err(run_store_error)?.is_empty() {
      return Ok(Some((
        "already_ran",
        Message::new("orchestrator.start.alreadyRan").with("task", key),
      )));
    }
    Ok(None)
  })
  .await?;
  match refusal {
    None => Ok(()),
    Some((code, message)) => Err(ToolError::refused(code, vec![message])),
  }
}

/// Whether the turn thinking in `chat` (its `entries`) is the one a run's success started, whose run it merged with
/// `merge_run` in this turn, and to which an order of the conversation applies.
fn merged_under_order(db: &ProjectDb, chat: ChatId, entries: &[ChatEntry]) -> Result<bool, StoreError> {
  let thinking = entries.iter().rev().find_map(|entry| match &entry.kind {
    EntryKind::Wait { wait, turn: Some(turn) } if turn.state.is_thinking() => Some((entry.seq, wait)),
    _ => None,
  });
  let Some((seq, wait)) = thinking else {
    return Ok(false);
  };
  if wait.kind != WaitKind::Ended(RunEnd::Succeeded) || wait.merging != Some(Merging::Merges) {
    return Ok(false);
  }
  let merged = entries.iter().filter(|entry| entry.seq > seq).any(|entry| {
    matches!(&entry.kind, EntryKind::Action(action)
      if action.tool == MERGE_RUN && action.outcome == ActionOutcome::Done)
  });
  if !merged {
    return Ok(false);
  }
  Ok(db.chat_leads(chat)?.ordered_through(db.came_through(chat, wait.run)?))
}
