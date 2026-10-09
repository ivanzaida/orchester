//! Runs whose stored record cannot be read back, as the task list and the task page show them: the
//! run's number and, in short, why. The record stays as it is in the project database; nothing can
//! change the run, so its actions are unavailable. The page of a task whose latest run cannot be
//! read lists the task's runs under its notice (02.R `SUuwV`, *Runs of this task* since ORC-63).

use orchester_core::{
  id::RunId,
  run::{RunNumber, UnreadableRun},
};
use orchester_store::{RunStoreError, StoreError};

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
