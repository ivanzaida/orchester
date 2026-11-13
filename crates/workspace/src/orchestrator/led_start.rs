//! What a turn Orchester started by itself may start while the chat drives (`docs/design/orchestrator-watch.md`,
//! routing; `docs/design/orchestrator-toolset.md`, decision 14). A conversation that only follows a task starts it
//! only in the turn of a run's success it merged, where an order of the conversation applies (`not_merge_turn` in any
//! other), and only while the task has no run yet (`already_ran`): running a task again is then the person's. A
//! conversation that leads a task to the end (`docs/plan/orchestrator-autonomy/OA-13-…`, `OA-14-…`) starts it in any
//! such turn, the one work that can start raises among them, and runs it again after a failure, with its reason,
//! while its ceiling of runs allows. Either way only a task its conversation leads (`not_led`,
//! [`orchester_core::orchestrator::Subscriptions::leading`]) and none that is parked for the person (`parked`). In a
//! turn a person's message started, any task of the project may be started.

use orchester_core::{
  id::ChatId,
  orchestrator::{
    ActionOutcome, ChatEntry, EntryKind, Merging, RunEnd, Subscription, WaitKind,
    change::{Change, ProposedChange},
  },
  run::RunStatus,
  store::RunStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  ceilings,
  team_status::module_now,
  tools::{ChatScope, MERGE_RUN, RUN_TASK},
};
use crate::{
  app_mcp::{ToolError, blocking, find_task},
  messages::Message,
  tasks::detail::run_store_error,
};

/// Refuses the chat's start of task `key` of `scope`'s project in a turn nobody asked for. A task its conversation
/// leads to the end is started in any such turn; one it only follows, only in the turn of a success it merged under an
/// order (`not_merge_turn`). Refused: a task parked for the person (`parked`), a task another conversation, or none,
/// leads (`not_led`), a task that has a run already (`already_ran`) unless it is led to the end and its latest run
/// failed: it is then run again while its ceiling of runs allows (`ceiling_reached` past it), which needs the chat's
/// reason (`reasoned`; `reason_required` without), and a start past the budget of what leads it (`ceiling_reached`). A
/// call that names no task is left to the start's own refusal.
pub(super) async fn led_here(scope: &ChatScope, key: Option<&str>, reasoned: bool) -> Result<(), ToolError> {
  let Some(named) = key.map(str::to_owned) else {
    return Ok(());
  };
  let (root, chat) = (scope.root.clone(), scope.chat);
  let refusal = blocking(move || {
    let db = ProjectDb::open(&root)?;
    let task = find_task(&db, &named)?;
    let key = task.key().to_string();
    let refused =
      |code: &'static str, message: &'static str| Ok(Some((code, Message::new(message).with("task", key.clone()))));
    if db.parked_task(task.id())?.is_some() {
      return refused("parked", "orchestrator.start.parked");
    }
    let subscriptions = db.active_subscriptions()?;
    let leader = subscriptions.leader_of(task.id(), module_now(&db, &task));
    let ceilings = leader
      .filter(|leader| leader.chat() == chat)
      .and_then(|leader| leader.mode().ceilings());
    if ceilings.is_none() && !merged_under_order(&db, chat, &db.chat_entries(chat)?)? {
      return Ok(Some((
        "not_merge_turn",
        Message::new("orchestrator.start.notMergeTurn"),
      )));
    }
    if leader.map(Subscription::chat) != Some(chat) {
      return refused("not_led", "orchestrator.start.notLed");
    }
    let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
    if let Some(latest) = runs.last() {
      let failed = latest
        .as_ref()
        .is_ok_and(|run| matches!(run.status(), RunStatus::Failed(_)));
      // Only a task led to the end, whose latest run failed, is run again from here, within its ceiling of runs.
      let Some(ceilings) = ceilings.filter(|_| failed) else {
        return refused("already_ran", "orchestrator.start.alreadyRan");
      };
      if let Some(reached) = ceilings::again_refused(ceilings, runs.len()) {
        return Ok(Some(("ceiling_reached", reached)));
      }
      if !reasoned {
        return Ok(Some((
          "reason_required",
          Message::new("orchestrator.tools.reasonRequired").with("tool", RUN_TASK.to_owned()),
        )));
      }
    }
    if let Some(spent) = ceilings::start_refused(&db, &task)? {
      return Ok(Some(("ceiling_reached", spent)));
    }
    Ok(None)
  })
  .await?;
  match refusal {
    None => Ok(()),
    Some((code, message)) => Err(ToolError::refused(code, vec![message])),
  }
}

/// Whether the turn thinking in `chat` (its `entries`) is the one a run's success started, which merged that very run
/// with `merge_run` in this turn, and to which an order of the conversation applied when its wait was kept.
fn merged_under_order(db: &ProjectDb, chat: ChatId, entries: &[ChatEntry]) -> Result<bool, StoreError> {
  let thinking = entries.iter().rev().find_map(|entry| match &entry.kind {
    EntryKind::Wait { wait, turn: Some(turn) } if turn.state.is_thinking() => Some((entry.seq, entry.at, wait)),
    _ => None,
  });
  let Some((seq, at, wait)) = thinking else {
    return Ok(false);
  };
  if wait.kind != WaitKind::Ended(RunEnd::Succeeded) || wait.merging != Some(Merging::Merges) {
    return Ok(false);
  }
  // Only the merge of the run whose success started the turn opens its starts: another run's merge does not.
  let merged = entries.iter().filter(|entry| entry.seq > seq).any(|entry| {
    matches!(&entry.kind, EntryKind::Action(action)
      if action.tool == MERGE_RUN
        && action.outcome == ActionOutcome::Done
        && matches!(action.change.as_deref(), Some(ProposedChange { change: Change::Merge { run, .. }, .. })
          if *run == wait.run))
  });
  if !merged {
    return Ok(false);
  }
  // Whether an order applied is read as it stood when the wait was kept: a subscription that ended since (its task
  // merged and done) does not take it back.
  db.ordered_when(chat, wait, at)
}
