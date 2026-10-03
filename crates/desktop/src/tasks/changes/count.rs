//! How many files the Changes tab lists, the count its label and the Conversation show.

use std::collections::HashSet;

use orchester_core::diff::FileDiff;

use super::{Checkpointed, NotCheckpointed};
use crate::messages::Message;

/// How many files the Changes tab lists for the run's `checkpointed` changes and what its
/// `worktree` holds beyond them (each `None` while it is being read): the files changed from the
/// run's base to its latest checkpoint and beyond it, a file changed in both counted once.
///
/// `None` while that is not known: either part is being read, could not be read, or the changes
/// not checkpointed are too large to list file by file; and before the first checkpoint while the
/// worktree holds nothing. Checkpointed changes too large to show count the files measured, since
/// the tab then shows only their size.
pub fn listed_files(
  checkpointed: Option<Result<&Checkpointed, &Message>>,
  worktree: Option<Result<&NotCheckpointed, &Message>>,
) -> Option<usize> {
  let checkpointed = checkpointed?.ok()?;
  let committed = match checkpointed {
    Checkpointed::TooLarge { .. } => return checkpointed.files(),
    Checkpointed::Read { changes, .. } => Some(changes),
    Checkpointed::NoCheckpoint => None,
  };
  let uncommitted = match worktree?.ok()? {
    NotCheckpointed::Read(changes) => Some(changes),
    NotCheckpointed::Clean | NotCheckpointed::Removed => None,
    NotCheckpointed::TooLarge { .. } => return None,
  };
  if committed.is_none() && uncommitted.is_none() {
    return None;
  }
  let paths: HashSet<&str> = committed
    .into_iter()
    .chain(uncommitted)
    .flat_map(|changes| changes.diff.files().iter().map(FileDiff::path))
    .collect();
  Some(paths.len())
}
