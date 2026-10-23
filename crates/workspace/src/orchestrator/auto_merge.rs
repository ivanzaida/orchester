//! The chat's rule for merging a finished run on its own (ORC-69, `docs/design/orchestrator-toolset.md`, decision 5).
//! With *Let the Orchestrator merge finished runs on its own* on, besides *merge runs* and *answer agents*, the turn a
//! task's run success starts by itself merges the run with `merge_run`, only when all of these hold ([`merging_in`]):
//! it is the task's latest run and succeeded; every step of its pipeline passed, a step accepted as it was counting
//! only when a person accepted it; the task does not say *Don't merge automatically*, is not marked Blocked and no
//! unfinished task blocks it; and new starts are not held for its reviewed group or the whole project. `merge_run`'s
//! own rules (the main checkout needs no local integration) hold on top, and a run the chat's own doing led to starts
//! no turn at all (`raised_by_own`). The turn merges only the run whose success started it, once per end
//! ([`merge_as`]); anything else in such a turn is proposed, as before.

use orchester_core::{
  id::RunId,
  orchestrator::{
    ActionOutcome, AgentWait, ChatAction, ChatEntry, EntryKind, MergeLeft, Merging, RunEnd, WaitKind, change::Change,
  },
  run::{Run, RunStatus, attempt_starts},
  store::{RunStore, TaskStore},
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::model::JsonObject;
use serde_json::Value;

use super::tools::{ChatScope, MERGE_RUN};
use crate::{
  app_mcp::{ToolError, blocking, find_run},
  messages::Message,
  tasks::{detail::run_store_error, hold},
};

/// Whether the turn of run `run`'s success merges the run on its own, or why it is left to the person. Blocks on
/// SQLite.
pub fn merging_in(db: &ProjectDb, run: RunId) -> Result<Merging, StoreError> {
  Ok(match left(db, run)? {
    Some(why) => Merging::Left { why },
    None => Merging::Merges,
  })
}

/// Why `run` is left to the person; `None` when the chat merges it on its own.
fn left(db: &ProjectDb, run: RunId) -> Result<Option<MergeLeft>, StoreError> {
  let Some(run) = db.run(run).map_err(run_store_error)? else {
    return Ok(Some(MergeLeft::NotReady));
  };
  let Some(task) = db.task(run.task())? else {
    return Ok(Some(MergeLeft::NotReady));
  };
  if !latest_succeeded(db, &run)? {
    return Ok(Some(MergeLeft::NotReady));
  }
  if crate::merge_queue::delivered_by_queue(db, &run)? {
    return Ok(Some(MergeLeft::DeliveredByQueue));
  }
  let organization = task.organization();
  if organization.no_automatic_merge() {
    return Ok(Some(MergeLeft::OptedOut));
  }
  let starts = attempt_starts(&db.events(run.id()).map_err(run_store_error)?);
  if let Some(steps) = MergeLeft::of_steps(&run, &starts) {
    return Ok(Some(steps));
  }
  if let Some(reason) = organization.blocked_reason() {
    return Ok(Some(MergeLeft::MarkedBlocked {
      reason: reason.to_owned(),
    }));
  }
  let mut blockers = Vec::new();
  for blocker in db.unfinished_blocking_tasks(task.id())? {
    if let Some(blocker) = db.task(blocker)? {
      blockers.push(blocker.key().to_string());
    }
  }
  if !blockers.is_empty() {
    return Ok(Some(MergeLeft::BlockedByTasks { tasks: blockers }));
  }
  let holds = db.queue_holds()?;
  let group_held = hold::group_of(db, task.id())?.is_some_and(|group| holds.groups.contains(&group));
  if holds.project || group_held {
    return Ok(Some(MergeLeft::Held));
  }
  Ok(None)
}

/// Whether `run` succeeded and no later run of its task exists, read or not.
fn latest_succeeded(db: &ProjectDb, run: &Run) -> Result<bool, StoreError> {
  if *run.status() != RunStatus::Succeeded {
    return Ok(false);
  }
  let number = run.number().get();
  let later = db
    .runs_of_task(run.task())
    .map_err(run_store_error)?
    .iter()
    // A later run that cannot be read is still a later run.
    .any(|listed| match listed {
      Ok(listed) => listed.number().get() > number,
      Err(unreadable) => unreadable.number.is_some_and(|other| other.get() > number),
    });
  Ok(!later)
}

/// What the chat's `merge_run` does in a turn nobody asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MergeAs {
  /// It proposes the merge to the person, as every merge in such a turn.
  Propose,
  /// It merges the run on its own, now.
  OnItsOwn,
}

/// What `merge_run` with `arguments` does in `scope`'s turn, which nobody asked for, when the person's switches let the
/// Orchestrator merge finished runs on its own (`switched_on`: that switch, *merge runs* and *answer agents*). It
/// merges only the run whose success started the turn, while the turn's wait says it merges it ([`Merging::Merges`]),
/// and only once: a second call in the turn is proposed. A run the rule leaves to the person now is refused
/// (`left_to_person`), with why; any other call is proposed.
pub(super) async fn merge_as(
  scope: &ChatScope,
  arguments: &JsonObject,
  switched_on: bool,
) -> Result<MergeAs, ToolError> {
  if !switched_on {
    return Ok(MergeAs::Propose);
  }
  let named = arguments
    .get("run")
    .and_then(Value::as_str)
    .ok_or_else(|| ToolError::InvalidArguments("run: required".to_owned()))?
    .to_owned();
  let (root, chat) = (scope.root.clone(), scope.chat);
  blocking(move || {
    let db = ProjectDb::open(&root)?;
    let run = find_run(&db, &named)?;
    if !started_to_merge(&db.chat_entries(chat)?, run.id()) {
      return Ok(MergeAs::Propose);
    }
    match left(&db, run.id())? {
      None => Ok(MergeAs::OnItsOwn),
      Some(why) => Err(ToolError::refused(
        "left_to_person",
        vec![left_message(&why, run.number().get())],
      )),
    }
  })
  .await
}

/// Whether, among a conversation's `entries`, the turn that thinks now is the one `run`'s success started to merge
/// it, and no merge of it was tried in that turn yet.
fn started_to_merge(entries: &[ChatEntry], run: RunId) -> bool {
  let Some((seq, wait)) = thinking_wait(entries) else {
    return false;
  };
  let merges =
    wait.run == run && wait.kind == WaitKind::Ended(RunEnd::Succeeded) && wait.merging == Some(Merging::Merges);
  merges
    && !entries
      .iter()
      .filter(|entry| entry.seq > seq)
      .any(|entry| matches!(&entry.kind, EntryKind::Action(action) if tried_to_merge(action, run)))
}

/// Whether `action` tried to merge `run`: a merge carried out or refused. A proposal merged nothing.
fn tried_to_merge(action: &ChatAction, run: RunId) -> bool {
  if action.tool != MERGE_RUN || !matches!(action.outcome, ActionOutcome::Done | ActionOutcome::Refused { .. }) {
    return false;
  }
  match action.change.as_deref().map(|change| &change.change) {
    Some(Change::Merge { run: merged, .. }) => *merged == run,
    // A merge refused before the tool knew its run (an unknown run, say) names none: it was a try all the same, so
    // the turn errs towards leaving the run to the person rather than trying twice.
    _ => true,
  }
}

/// The wait among a conversation's `entries` whose turn thinks now, with its entry: the turn under way, when a wait
/// started it.
fn thinking_wait(entries: &[ChatEntry]) -> Option<(i64, &AgentWait)> {
  entries.iter().rev().find_map(|entry| match &entry.kind {
    EntryKind::Wait { wait, turn: Some(turn) } if turn.state.is_thinking() => Some((entry.seq, wait)),
    _ => None,
  })
}

/// Why run `#number` is left to the person, in the words the chat's agent and the person read.
pub fn left_message(why: &MergeLeft, number: u32) -> Message {
  let run = number.to_string();
  match why {
    MergeLeft::NotReady => Message::new("orchestrator.tools.mergeLeft.notReady").with("run", run),
    MergeLeft::StepNotPassed { step } => Message::new("orchestrator.tools.mergeLeft.stepNotPassed")
      .with("run", run)
      .with("step", step.clone()),
    MergeLeft::AcceptedByAutomation { step } => Message::new("orchestrator.tools.mergeLeft.acceptedByAutomation")
      .with("run", run)
      .with("step", step.clone()),
    MergeLeft::MarkedBlocked { reason } => Message::new("orchestrator.tools.mergeLeft.markedBlocked")
      .with("run", run)
      .with("reason", reason.clone()),
    MergeLeft::BlockedByTasks { tasks } => Message::new("orchestrator.tools.mergeLeft.blockedByTasks")
      .with("run", run)
      .with("tasks", tasks.join(", ")),
    MergeLeft::Held => Message::new("orchestrator.tools.mergeLeft.held").with("run", run),
    MergeLeft::OptedOut => Message::new("orchestrator.tools.mergeLeft.optedOut").with("run", run),
    MergeLeft::DeliveredByQueue => Message::new("orchestrator.tools.mergeLeft.deliveredByQueue").with("run", run),
  }
}
