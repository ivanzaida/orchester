//! A run's changes with what its branch took in from branches other than its target ([`TakenIn`]): the Changes tab
//! says so over the diff, which is what merging the run brings, and offers its agent's own work alone, read from its
//! start merged with what it took in. The Orchester chat leaves a run that took any in to the person to merge.

use std::path::Path;

use orchester_core::integration::{TakenIn, taken_in, without_taken_in};
use orchester_exec::programs::Programs;

use super::{ChangesError, Checkpointed, GIT, RunRange, checkpointed_in, own_start};

/// The run's changes as the Changes tab shows them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunChangesView {
  pub checkpointed: Checkpointed,
  /// What its branch took in from other branches; `None` when nothing, or when git could not tell.
  pub taken_in: Option<TakenIn>,
  /// The changes are its agent's own, without what it took in; `false` when they are all a merge brings, and when the
  /// own ones were asked for but its start and what it took in conflict, so they cannot be told apart.
  pub own_only: bool,
}

/// The run's changes up to its latest checkpoint ([`super::run_changes_in`]) with what its branch took in from other
/// branches; with `own_only`, while it took any in, from its start merged with that, so only its agent's work shows.
/// Blocks on SQLite and git.
pub fn run_changes_view_in(
  repo_root: &Path,
  programs: &Programs,
  (range, own_only): (&RunRange, bool),
) -> Result<RunChangesView, ChangesError> {
  let Some(head) = &range.head else {
    return Ok(RunChangesView {
      checkpointed: Checkpointed::NoCheckpoint,
      taken_in: None,
      own_only: false,
    });
  };
  let start = own_start(repo_root, programs, range, head)?;
  let runner = programs.runner();
  let git = (Path::new(GIT), repo_root);
  // What git cannot tell of it is left out: the diff is read as it always was.
  let taken = taken_in(&runner, git, (&range.work, &start, head)).unwrap_or_else(|error| {
    tracing::warn!(%error, run = %range.run, "cannot tell what the run's branch took in from other branches");
    None
  });
  let own = match (&taken, own_only) {
    (Some(taken), true) => without_taken_in(&runner, git, (&start, &taken.tip)).unwrap_or_else(|error| {
      tracing::warn!(%error, run = %range.run, "cannot merge the run's start with what its branch took in");
      None
    }),
    _ => None,
  };
  let from = own.as_ref().unwrap_or(&start);
  Ok(RunChangesView {
    checkpointed: checkpointed_in(repo_root, programs, from, Some(head))?,
    taken_in: taken,
    own_only: own.is_some(),
  })
}

/// What the branch of the run `range` reads took in from branches other than its target, at its latest checkpoint;
/// `None` when nothing, before its first checkpoint, and when git cannot tell. Blocks on SQLite and git.
pub fn run_taken_in(repo_root: &Path, programs: &Programs, range: &RunRange) -> Option<TakenIn> {
  let head = range.head.as_ref()?;
  let start = own_start(repo_root, programs, range, head).ok()?;
  taken_in(
    &programs.runner(),
    (Path::new(GIT), repo_root),
    (&range.work, &start, head),
  )
  .inspect_err(|error| tracing::warn!(%error, run = %range.run, "cannot tell what the run's branch took in"))
  .ok()
  .flatten()
}
