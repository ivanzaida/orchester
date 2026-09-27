//! A stored run that cannot be read back: a listing of runs reports it beside the runs it could
//! read instead of failing as a whole, so one damaged record never hides the others. The store
//! keeps the record unchanged; nothing writes over or repairs it.

use super::{number::RunNumber, state::Run};
use crate::id::{RunId, TaskId};

/// A run whose stored record a listing could not read back. Its identity was read; the rest of the
/// record breaks the run's rules or cannot be decoded, and `error` says why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnreadableRun<E> {
  pub id: RunId,
  pub task: TaskId,
  /// `None` when the stored number is not a run number either.
  pub number: Option<RunNumber>,
  /// The error reading the run by its id returns too.
  pub error: E,
}

/// One run of a listing: the run, or the stored run that could not be read back.
pub type ListedRun<E> = Result<Run, UnreadableRun<E>>;
