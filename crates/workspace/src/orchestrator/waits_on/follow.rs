//! What ends a run's wait on a task, as the waits follower reads it from stored state (`super::super::waits`): the
//! waited task's run merged into the waiting run's base branch, the waited task canceled or closed without that merge,
//! the waiting run ended, or the waiting run answered first, on its own wait card ([`settle`]). A wait the waited task
//! ended is an event of the waiting task ([`event`]), kept in the conversation that leads it and starting a turn there
//! by itself as a wait does; it stands while the run still waits for the person and nobody answered it since.

use orchester_core::{
  conversation::{Message, Sender},
  id::{RunId, TaskId},
  orchestrator::{TaskEvent, TaskEventKind},
  run::{Run, RunWaitsOn, WaitsOnChange},
  store::{ConversationStore, MergeStore, RunStore},
  time::Timestamp,
};
use orchester_store::{KeptRunWait, ProjectDb, RunWaitEnd, StoreError};

use crate::tasks::{detail::run_store_error, recorded::task_of};

/// Ends each active wait of the project in `db` that something ended, at `now`. Writes only for a wait that ends.
pub(in crate::orchestrator) fn settle(db: &mut ProjectDb, now: Timestamp) -> Result<(), StoreError> {
  for kept in db.active_run_waits()? {
    if let Some(end) = end_of(db, &kept)? {
      db.end_run_wait(kept.wait.run(), end, now)?;
    }
  }
  Ok(())
}

/// How `kept` ends now, if it does: what became of its run and the waited task first, then whether the run was
/// answered since it began to wait.
fn end_of(db: &ProjectDb, kept: &KeptRunWait) -> Result<Option<RunWaitEnd>, StoreError> {
  let wait = &kept.wait;
  // A run is never deleted while its waits are kept; one that cannot be found waits for no one.
  let Some(run) = db.run(wait.run()).map_err(run_store_error)? else {
    return Ok(Some(RunWaitEnd::RunEnded));
  };
  let mut changes = Vec::new();
  if run.status().is_terminal() {
    changes.push(WaitsOnChange::RunEnded { run: run.id() });
  }
  if merged(db, wait)?.is_some() {
    changes.push(WaitsOnChange::Merged {
      task: wait.waited(),
      branch: wait.base().clone(),
    });
  }
  if let Some(task) = task_of(db, wait.waited())? {
    changes.push(WaitsOnChange::StatusChanged {
      task: task.id(),
      status: task.organization().status(),
    });
  }
  if let Some(end) = changes.iter().find_map(|change| wait.end(change)) {
    return Ok(Some(RunWaitEnd::from(&end)));
  }
  let answered = !waits_for_person(&run) || answered_since(&db.messages(run.task())?, run.id(), kept.since);
  Ok(answered.then_some(RunWaitEnd::Answered))
}

/// The event of `run`, a run that waits for the person, whose latest wait the waited task ended, by the key it is
/// kept by, with its run's `messages`; `None` when its wait did not end so, or someone answered the run since.
pub(in crate::orchestrator) fn event(
  db: &ProjectDb,
  run: &Run,
  messages: &[Message],
) -> Result<Option<(String, TaskEvent)>, StoreError> {
  let Some(kept) = db.run_wait(run.id())? else {
    return Ok(None);
  };
  let Some(ended) = kept.ended else {
    return Ok(None);
  };
  let waited = kept.wait.waited();
  let kind = match ended.end {
    RunWaitEnd::Merged => match merged(db, &kept.wait)? {
      Some(merged) => TaskEventKind::WaitedMerged {
        task: waited,
        run: merged,
      },
      None => return Ok(None),
    },
    RunWaitEnd::Canceled | RunWaitEnd::ClosedUnmerged => TaskEventKind::WaitedClosed {
      task: waited,
      run: latest_run(db, waited)?,
    },
    RunWaitEnd::RunEnded | RunWaitEnd::Answered | RunWaitEnd::Dropped => return Ok(None),
  };
  if answered_since(messages, run.id(), ended.at) {
    return Ok(None);
  }
  let key = format!("waited:{}:{}", run.id(), kept.since.unix_millis());
  Ok(Some((key, TaskEvent { kind, at: ended.at })))
}

/// The run of the waited task whose work was merged into the waiting run's base branch, if one was.
fn merged(db: &ProjectDb, wait: &RunWaitsOn) -> Result<Option<RunId>, StoreError> {
  for run in runs_of(db, wait.waited())? {
    if db.run_merge(run)?.is_some_and(|merge| merge.into == *wait.base()) {
      return Ok(Some(run));
    }
  }
  Ok(None)
}

/// The latest run of `task`, if it has one.
fn latest_run(db: &ProjectDb, task: TaskId) -> Result<Option<RunId>, StoreError> {
  Ok(runs_of(db, task)?.pop())
}

/// The runs of `task`, oldest first, by their ids: one whose record cannot be read back still has its merge kept.
fn runs_of(db: &ProjectDb, task: TaskId) -> Result<Vec<RunId>, StoreError> {
  Ok(
    db.runs_of_task(task)
      .map_err(run_store_error)?
      .into_iter()
      .map(|listed| listed.map_or_else(|unreadable| unreadable.id, |run| run.id()))
      .collect(),
  )
}

/// Whether `run` still waits for a message or a decision on its plan.
pub(super) fn waits_for_person(run: &Run) -> bool {
  let waits = !run.awaiting_answer().is_empty() || !run.awaiting_plan().is_empty();
  waits && !run.status().is_terminal()
}

/// Whether a person, or the Orchestrator for them, sent `run` a message among `messages` at `since` or after.
pub(super) fn answered_since(messages: &[Message], run: RunId, since: Timestamp) -> bool {
  messages.iter().any(|message| {
    message.run() == Some(run) && matches!(message.sender(), Sender::Person { .. }) && message.created_at() >= since
  })
}
