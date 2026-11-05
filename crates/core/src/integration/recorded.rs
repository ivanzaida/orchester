//! Removing a run's worktrees with the removal recorded first (ORC-63): the store refuses the
//! removal of a run that has live work as stored now, and a continuation of a run whose removal is
//! recorded, so a person removing a worktree and the run continuing in it never both happen. Should
//! nothing be removed because changes appeared after the person saw them, the record is taken back
//! and the run can continue as before. It stays once anything was removed, even when its own
//! worktree stayed (a step it would continue from may be gone), and once git refused a worktree:
//! a refusal can come part way, leaving a worktree git forgot (`super::forgotten`), which no agent
//! can continue in.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Cleanup, CleanupError, Leftovers, clean_up};
use crate::{command::CommandRunner, id::RunId, run::Run, store::WorktreeStore, time::Timestamp};

/// What recording a removal found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemovalStart {
  /// Recorded now; it is taken back if git removes nothing.
  Recorded,
  /// A removal was recorded before and stays.
  AlreadyRecorded,
  /// The run has live work: nothing was recorded.
  LiveWork,
  /// Local integration or an unfinished Git process still needs this worktree.
  LocalIntegration,
}

#[derive(Debug, Error)]
pub enum RecordedCleanupError<E: std::error::Error + 'static> {
  #[error(transparent)]
  Cleanup(#[from] CleanupError),
  #[error("cannot record the removal: {0}")]
  Store(#[source] E),
  /// The run as stored once the removal was recorded is not the one the removal was asked for:
  /// nothing was removed, and a removal recorded now was taken back.
  #[error("the run changed since its worktrees were listed")]
  RunChanged,
}

/// Removes what `run` left in the repository at `repo` as [`clean_up`] does, `confirmed` being what
/// a person saw, with the removal recorded in `store` at `at` first.
pub fn clean_up_recorded<S: WorktreeStore>(
  store: &mut S,
  git: (&impl CommandRunner, &Path, &Path),
  run: &Run,
  confirmed: Option<&Leftovers>,
  at: Timestamp,
) -> Result<Cleanup, RecordedCleanupError<S::Error>> {
  clean_up_recorded_as_seen(store, git, run, confirmed, (at, |_: &S| Ok(true)))
}

/// [`clean_up_recorded`], only while `unchanged` says the run is still as `run` shows it, read once the removal
/// is recorded, so the run cannot change after the answer: otherwise nothing is removed
/// ([`RecordedCleanupError::RunChanged`]).
pub fn clean_up_recorded_as_seen<S: WorktreeStore>(
  store: &mut S,
  (runner, git, repo): (&impl CommandRunner, &Path, &Path),
  run: &Run,
  confirmed: Option<&Leftovers>,
  (at, unchanged): (Timestamp, impl FnOnce(&S) -> Result<bool, S::Error>),
) -> Result<Cleanup, RecordedCleanupError<S::Error>> {
  let start = record_removal_as_seen(store, run.id(), (at, unchanged))?;
  let cleanup = clean_up(runner, git, repo, run, confirmed);
  // A clean-up fails only before it removes anything (`clean_up`): nothing was touched.
  settle_removal(store, (run.id(), start, at), cleanup.as_ref().ok()).map_err(RecordedCleanupError::Store)?;
  Ok(cleanup?)
}

/// Records in `store` at `at` the removal of `run`'s worktrees before anything is removed, only while `unchanged` says
/// the run is still as the one asking saw it, read once the removal is recorded, so the run cannot change after the
/// answer: otherwise a record made now is taken back ([`RecordedCleanupError::RunChanged`]). Refused, with nothing
/// recorded, for a run with live work or one local integration holds. Whoever removed then ends it with
/// [`settle_removal`]; the removal itself may happen elsewhere (the runner, [`super::removal`]).
pub fn record_removal_as_seen<S: WorktreeStore>(
  store: &mut S,
  run: RunId,
  (at, unchanged): (Timestamp, impl FnOnce(&S) -> Result<bool, S::Error>),
) -> Result<RemovalStart, RecordedCleanupError<S::Error>> {
  let start = store
    .begin_worktree_removal(run, at)
    .map_err(RecordedCleanupError::Store)?;
  if start == RemovalStart::LiveWork {
    return Err(CleanupError::LiveWork.into());
  }
  if start == RemovalStart::LocalIntegration {
    return Err(CleanupError::LocalIntegration.into());
  }
  if !unchanged(store).map_err(RecordedCleanupError::Store)? {
    if start == RemovalStart::Recorded {
      store
        .undo_worktree_removal(run, at)
        .map_err(RecordedCleanupError::Store)?;
    }
    return Err(RecordedCleanupError::RunChanged);
  }
  Ok(start)
}

/// Takes back the removal of `run`'s worktrees recorded now (`start` at `at`, [`record_removal_as_seen`]) when what
/// removing did, `cleanup` (`None`: it failed before it removed anything), touched nothing ([`Cleanup::untouched`]).
pub fn settle_removal<S: WorktreeStore>(
  store: &mut S,
  (run, start, at): (RunId, RemovalStart, Timestamp),
  cleanup: Option<&Cleanup>,
) -> Result<(), S::Error> {
  if start == RemovalStart::Recorded && cleanup.is_none_or(Cleanup::untouched) {
    store.undo_worktree_removal(run, at)?;
  }
  Ok(())
}
