//! Runs whose stored record cannot be read back, as the task list and the task page show them: the
//! run's number and, in short, why. The record stays as it is in the project database; nothing can
//! change the run, so its actions are unavailable. The page of a task whose latest run cannot be
//! read lists the task's other runs instead (02.R `SUuwV`).

use orchester_core::{
  id::RunId,
  resource::StepKey,
  run::{ListedRun, Run, RunFailure, RunNumber, RunStatus, TaskPhase, UnreadableRun},
  store::ActivityStore,
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use super::data::latest_run;
use crate::messages::Message;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unreadable {
  pub id: RunId,
  /// `None` when its stored number cannot be read either.
  pub number: Option<RunNumber>,
  /// Why it cannot be read, in short, and that its record is kept unchanged.
  pub detail: Message,
}

impl Unreadable {
  /// `run` as the UI shows it; logs why it cannot be read, with its id.
  pub fn of(run: &UnreadableRun<RunStoreError>) -> Self {
    tracing::warn!(run = %run.id, error = %run.error, "a run of the project cannot be read");
    Self {
      id: run.id,
      number: run.number,
      detail: detail(&run.error),
    }
  }

  /// "Run #9 could not be read".
  pub fn title(&self) -> Message {
    match self.number {
      Some(number) => Message::new("runs.unreadable.title").with("run", number.get().to_string()),
      None => Message::new("runs.unreadable.titleUnnumbered"),
    }
  }

  /// Why the run's actions are unavailable (`AZI67` in 02.R).
  pub fn actions(&self) -> Message {
    match self.number {
      Some(number) => Message::new("runs.unreadable.actions").with("run", number.get().to_string()),
      None => Message::new("runs.unreadable.actionsUnnumbered"),
    }
  }
}

/// A run of a task whose latest run cannot be read, as the list of its other runs shows it (`GHwIe`
/// in 02.R): its number, where it stands, what happened and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EarlierRun {
  pub number: Option<RunNumber>,
  /// `None` when it cannot be read either.
  pub phase: Option<TaskPhase>,
  pub detail: Message,
  /// When it last changed; `None` when it cannot be read.
  pub at: Option<Timestamp>,
}

impl EarlierRun {
  /// `run`, a run of `task`, as the list shows it.
  pub(super) fn read(db: &ProjectDb, task: &Task, run: &ListedRun<RunStoreError>) -> Result<Self, StoreError> {
    let run = match run {
      Ok(run) => run,
      Err(unreadable) => {
        let unreadable = Unreadable::of(unreadable);
        return Ok(Self {
          number: unreadable.number,
          phase: None,
          detail: unreadable.title(),
          at: None,
        });
      }
    };
    let latest = latest_run(db, task, run)?;
    let detail = match run.status() {
      RunStatus::Failed(RunFailure::StepFailed(step) | RunFailure::RetriesExhausted(step)) => {
        Message::new("runs.unreadable.otherRuns.failedAt").with("step", step_name(run, step))
      }
      RunStatus::Cancelled => cancelled(db, run)?,
      _ => latest.message(),
    };
    Ok(Self {
      number: Some(run.number()),
      phase: Some(latest.phase),
      detail,
      at: Some(latest.updated_at),
    })
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

/// "Canceled at Implement · work kept on orc/repo-switcher", at the step of the run's last agent or
/// command, if one ran.
fn cancelled(db: &ProjectDb, run: &Run) -> Result<Message, StoreError> {
  if run.current_attempt().is_none() {
    return Ok(Message::new("runs.unreadable.otherRuns.cancelledBeforeStart"));
  }
  let branch = run.branch().as_str().to_owned();
  let step = db
    .invocations(run.id())?
    .last()
    .and_then(|invocation| run.execution(invocation.execution))
    .map(|execution| step_name(run, execution.step()));
  Ok(match step {
    Some(step) => Message::new("runs.unreadable.otherRuns.cancelledAt")
      .with("step", step)
      .with("branch", branch),
    None => Message::new("runs.unreadable.otherRuns.cancelled").with("branch", branch),
  })
}

/// `error` in short: a stored value is invalid or unknown to this version, or the database could
/// not read the record; either way the record is kept unchanged.
fn detail(error: &RunStoreError) -> Message {
  match error {
    RunStoreError::Unreadable {
      source: StoreError::Corrupt { .. },
      ..
    } => Message::new("runs.unreadable.detail.invalid"),
    _ => Message::new("runs.unreadable.detail.database"),
  }
}
