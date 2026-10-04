//! A task's runs as *Runs of this task* lists them on its Summary (ORC-63, 63.01 `O0AMKY`): each
//! run's number, where it stands, what happened, when it last changed and when its own worktree
//! was removed. A run that cannot be read is listed with its number alone; nothing can change it.

use std::sync::Arc;

use orchester_core::{
  id::RunId,
  resource::StepKey,
  run::{ListedRun, Run, RunFailure, RunNumber, RunStatus, TaskPhase},
  store::{ActivityStore, WorktreeStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use super::{data::latest_run, unreadable::Unreadable};
use crate::messages::Message;

/// One run of the task, as its row shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRecord {
  pub id: RunId,
  /// `None` when its stored number cannot be read either.
  pub number: Option<RunNumber>,
  /// Where it stands; `None` when it cannot be read.
  pub phase: Option<TaskPhase>,
  /// What happened: "Failed at Review", "Run #9 could not be read".
  pub detail: Message,
  /// When it last changed; `None` when it cannot be read.
  pub at: Option<Timestamp>,
  /// The run as stored; `None` when it cannot be read.
  pub run: Option<Arc<Run>>,
  /// When its own worktree was removed, if Orchester removed it.
  pub removed_at: Option<Timestamp>,
}

impl RunRecord {
  /// The run, when it can be read and nothing works in its worktrees any more: it ended or was
  /// interrupted, so a person may remove them.
  pub fn without_live_work(&self) -> Option<&Arc<Run>> {
    self.run.as_ref().filter(|run| !run.status().has_live_work())
  }

  fn unreadable(run: &Unreadable) -> Self {
    Self {
      id: run.id,
      number: run.number,
      phase: None,
      detail: run.title(),
      at: None,
      run: None,
      removed_at: None,
    }
  }
}

/// The runs of `task`, `listed` oldest first as the store lists them, newest first.
pub(super) fn read(
  db: &ProjectDb,
  task: &Task,
  listed: &[ListedRun<RunStoreError>],
) -> Result<Vec<RunRecord>, StoreError> {
  let removals = db.worktree_removals(task.id())?;
  let removed = |run: RunId| removals.iter().find(|(removed, _)| *removed == run).map(|(_, at)| *at);
  listed
    .iter()
    .rev()
    .map(|listed| match listed {
      Ok(run) => record(db, task, run, removed(run.id())),
      Err(unreadable) => Ok(RunRecord::unreadable(&Unreadable::of(unreadable))),
    })
    .collect()
}

fn record(db: &ProjectDb, task: &Task, run: &Run, removed_at: Option<Timestamp>) -> Result<RunRecord, StoreError> {
  let latest = latest_run(db, task, run)?;
  let step = || last_step(db, run);
  let detail = match run.status() {
    RunStatus::Failed(RunFailure::StepFailed(step) | RunFailure::RetriesExhausted(step)) => {
      Message::new("runs.history.failedAt").with("step", step_name(run, step))
    }
    RunStatus::Cancelled if run.current_attempt().is_none() => Message::new("runs.history.cancelledBeforeStart"),
    RunStatus::Cancelled => match step()? {
      Some(step) => Message::new("runs.history.cancelledAt").with("step", step),
      None => Message::new("runs.history.cancelled"),
    },
    RunStatus::Interrupted(_) => interrupted(step()?, removed_at.is_some()),
    _ => latest.message(),
  };
  Ok(RunRecord {
    id: run.id(),
    number: Some(run.number()),
    phase: Some(latest.phase),
    detail,
    at: Some(latest.updated_at),
    run: Some(Arc::new(run.clone())),
    removed_at,
  })
}

/// "Interrupted at Implement", and once its worktree was removed, that it can't continue.
fn interrupted(step: Option<String>, removed: bool) -> Message {
  match (step, removed) {
    (Some(step), false) => Message::new("runs.history.interruptedAt").with("step", step),
    (Some(step), true) => Message::new("runs.history.interruptedAtCantContinue").with("step", step),
    (None, false) => Message::new("runs.history.interrupted"),
    (None, true) => Message::new("runs.history.interruptedCantContinue"),
  }
}

/// The name of `step` in the pipeline `run` pinned.
fn step_name(run: &Run, step: &StepKey) -> String {
  run
    .snapshot()
    .body()
    .step(step)
    .map_or_else(|| step.as_str().to_owned(), |step| step.name.as_str().to_owned())
}

/// The step of the run's last agent or command, if one ran.
fn last_step(db: &ProjectDb, run: &Run) -> Result<Option<String>, StoreError> {
  Ok(
    db.invocations(run.id())?
      .last()
      .and_then(|invocation| run.execution(invocation.execution))
      .map(|execution| step_name(run, execution.step())),
  )
}
