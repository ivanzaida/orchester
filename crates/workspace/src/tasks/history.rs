//! A task's runs as *Runs of this task* lists them on its Summary (ORC-63, 63.01 `O0AMKY`): each
//! run's number, where it stands, what happened, when it last changed and when its own worktree
//! was removed. A run that cannot be read is listed with its number alone; nothing can change it.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::RunId,
  resource::{AgentProfile, StepKey},
  run::{ListedRun, Run, RunFailure, RunNumber, RunStatus, StepExecutor, StepStatus, TaskPhase, attempt_starts},
  store::{ActivityStore, ResourceStore, RunStore, WorktreeStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use super::{changes::run_worktree, data::latest_run, detail::run_store_error, unreadable::Unreadable};
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

/// Records the removal of the worktree of each interrupted run of `listed` whose folder is gone
/// (deleted by hand): it can never continue, so it no longer holds its task. The time is when it was
/// found gone; a run that continued meanwhile has live work and is left alone.
pub(super) fn note_missing_worktrees(
  db: &mut ProjectDb,
  repo_root: &Path,
  listed: &[ListedRun<RunStoreError>],
) -> Result<(), StoreError> {
  let interrupted = listed
    .iter()
    .flatten()
    .filter(|run| matches!(run.status(), RunStatus::Interrupted(_)));
  for run in interrupted {
    if db.worktree_removal(run.id())?.is_none() && !run_worktree(repo_root, run.id()).is_dir() {
      tracing::warn!(run = %run.id(), "the interrupted run's worktree is gone; it can no longer continue");
      db.begin_worktree_removal(run.id(), Timestamp::now())?;
    }
  }
  Ok(())
}

/// The runs of `task` of project database `db` as its Summary lists them, newest first. Only reads: the worktrees
/// of interrupted runs found gone are noted when the task's page reads it.
pub fn runs_of(db: &ProjectDb, task: &Task) -> Result<Vec<RunRecord>, StoreError> {
  let listed = db.runs_of_task(task.id()).map_err(run_store_error)?;
  read(db, task, &listed)
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
  let detail = match (run.status(), latest.phase) {
    (RunStatus::Failed(RunFailure::StepFailed(step) | RunFailure::RetriesExhausted(step)), _) => {
      Message::new("runs.history.failedAt").with("step", step_name(run, step))
    }
    (RunStatus::Failed(RunFailure::WorktreeNotCreated(_)), _) => Message::new("runs.history.worktreeNotCreated"),
    (RunStatus::Cancelled, _) if run.current_attempt().is_none() => Message::new("runs.history.cancelledBeforeStart"),
    (RunStatus::Cancelled, _) => match step()? {
      Some(step) => Message::new("runs.history.cancelledAt").with("step", step),
      None => Message::new("runs.history.cancelled"),
    },
    (RunStatus::Interrupted(_), _) => interrupted(step()?, removed_at.is_some()),
    (RunStatus::Running, TaskPhase::Running) if !run.pause_requested() => at_work(db, run)?,
    (RunStatus::Succeeded, TaskPhase::Succeeded) => Message::new("runs.history.notMerged"),
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

/// The step of the run's last agent or command, else the first step of its attempt that did not
/// pass, if any.
pub(crate) fn last_step(db: &ProjectDb, run: &Run) -> Result<Option<String>, StoreError> {
  let invoked = db
    .invocations(run.id())?
    .last()
    .and_then(|invocation| run.execution(invocation.execution))
    .map(|execution| step_name(run, execution.step()));
  Ok(invoked.or_else(|| {
    run
      .current_attempt()?
      .executions()
      .iter()
      .find(|execution| execution.status() != StepStatus::Passed)
      .map(|execution| step_name(run, execution.step()))
  }))
}

/// "Implement · Builder at work": the first step an agent of the run works on, with the agent role
/// of its profile version while that exists; and when and with which branch its work was returned to it to resolve a
/// merge's conflicts.
fn at_work(db: &ProjectDb, run: &Run) -> Result<Message, StoreError> {
  let working = run.current_attempt().and_then(|attempt| {
    attempt
      .executions()
      .iter()
      .find(|execution| matches!(execution.status(), StepStatus::Running | StepStatus::Waiting(_)))
  });
  let Some(execution) = working else {
    return Ok(Message::new("tasks.progress.running"));
  };
  let step = step_name(run, execution.step());
  let role = match run.snapshot().step(execution.step()).map(|pinned| pinned.executor()) {
    Some(StepExecutor::Agent { profile, .. }) => db
      .version::<AgentProfile>(profile.resource, profile.number)?
      .map(|version| version.body().role().as_str().to_owned()),
    _ => None,
  };
  if let Some(resolving) = resolving(db, run)? {
    let message = match role {
      Some(agent) => Message::new("runs.history.atWorkResolving").with("agent", agent),
      None => Message::new("runs.history.stepAtWorkResolving"),
    };
    return Ok(
      message
        .with("step", step)
        .with("time", resolving.0)
        .with("branch", resolving.1),
    );
  }
  Ok(match role {
    Some(agent) => Message::new("runs.history.atWork")
      .with("step", step)
      .with("agent", agent),
    None => Message::new("runs.history.stepAtWork").with("step", step),
  })
}

/// When the run's work was returned to resolve a merge's conflicts, and with which branch, while its step works on
/// it ([`super::return_work::resolving_conflicts`]).
fn resolving(db: &ProjectDb, run: &Run) -> Result<Option<(String, String)>, StoreError> {
  let (Some((_, conflict)), Some(attempt)) = (super::return_work::resolving_conflicts(run), run.current_attempt())
  else {
    return Ok(None);
  };
  let started = attempt_starts(&db.events(run.id()).map_err(run_store_error)?)
    .into_iter()
    .find(|start| start.attempt == attempt.id())
    .map(|start| crate::display::clock(start.at));
  Ok(started.map(|time| (time, conflict.base)))
}
