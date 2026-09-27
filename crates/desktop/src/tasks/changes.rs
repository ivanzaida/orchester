//! A run's changes as the task page's Changes tab shows them: its diff from its base revision to
//! its latest checkpoint, read with git once per pair of commits and stored, and what its worktree
//! (`<repo>/.orchester/worktrees/<run>`) holds beyond its commit, which is not checkpointed and is
//! read each time. Only read-only git commands run. Blocks on SQLite, blob files and git; the UI
//! calls it off the UI thread.

use std::{
  hash::{DefaultHasher, Hash, Hasher},
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  command::CommandError,
  diff::{
    Changes, Diff, DiffParseError,
    git::{GitDiffError, RevisionChanges, RevisionDiffError, WorktreeRead, read_revision_diff, read_worktree_changes},
  },
  id::RunId,
  integration::WORKTREES_DIR,
  revision::Revision,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use crate::{messages::Message, programs::Programs};

/// The `git` program, looked up on the `PATH` like the runner does.
pub const GIT: &str = "git";

/// The worktree of `run` in the repository at `repo_root`.
pub fn run_worktree(repo_root: &Path, run: RunId) -> PathBuf {
  repo_root.join(WORKTREES_DIR).join(run.to_string())
}

/// A diff as read, parsed, with the key that tells its content apart from other diffs.
#[derive(Debug, PartialEq, Eq)]
pub struct ReadChanges {
  /// The revision the changes start from.
  pub from: Revision,
  pub diff: Arc<Diff>,
  pub key: Arc<str>,
}

/// The run's changes from its base to its latest checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checkpointed {
  /// The run has no checkpoint yet: nothing of its work is committed.
  NoCheckpoint,
  /// The diff up to `head`, the latest checkpoint.
  Read { head: Revision, changes: Arc<ReadChanges> },
  /// Larger than Orchester reads (the pull request limits); only its size was read.
  TooLarge { head: Revision, size: Changes },
}

impl lurq::app::component::DevtoolsInspectable for Checkpointed {}

impl Checkpointed {
  /// How many files it changes, when that is known.
  pub fn files(&self) -> Option<usize> {
    match self {
      Self::NoCheckpoint => None,
      Self::Read { changes, .. } => Some(changes.diff.files().len()),
      Self::TooLarge { size, .. } => Some(usize::try_from(size.files).expect("a u32 fits in usize on 32 and 64 bit")),
    }
  }
}

/// What the run's worktree holds beyond its checked-out commit: work of an agent that is not
/// checkpointed yet, or that was kept when a run stopped without its checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotCheckpointed {
  /// Nothing beyond its commit, or no worktree yet.
  Clean,
  /// The run ended and its worktree was removed; its checkpoints are all there is.
  Removed,
  Read(Arc<ReadChanges>),
  TooLarge {
    from: Revision,
    size: Changes,
  },
}

impl lurq::app::component::DevtoolsInspectable for NotCheckpointed {}

#[derive(Debug, Error)]
pub enum ChangesError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Git(#[from] GitDiffError),
  #[error("git's diff is not a unified diff: {0}")]
  Diff(#[from] DiffParseError),
}

impl From<RevisionDiffError<StoreError>> for ChangesError {
  fn from(error: RevisionDiffError<StoreError>) -> Self {
    match error {
      RevisionDiffError::Git(error) => Self::Git(error),
      RevisionDiffError::Store(error) => Self::Store(error),
    }
  }
}

impl ChangesError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => crate::messages::store_error(error),
      Self::Git(GitDiffError::Run(CommandError::NotFound)) => Message::new("runs.changes.failure.gitMissing"),
      Self::Git(_) | Self::Diff(_) => Message::new("runs.changes.failure.git"),
    }
  }
}

/// The run's changes from `base` to `head`, its latest checkpoint (`None` before its first).
pub fn checkpointed_in(
  repo_root: &Path,
  programs: &Programs,
  base: &Revision,
  head: Option<&Revision>,
) -> Result<Checkpointed, ChangesError> {
  let Some(head) = head else {
    return Ok(Checkpointed::NoCheckpoint);
  };
  let mut db = ProjectDb::open(repo_root)?;
  let read = read_revision_diff(
    &programs.runner(),
    Path::new(GIT),
    repo_root,
    &mut db,
    (base, head),
    Timestamp::now(),
  )?;
  Ok(match read {
    RevisionChanges::Read(read) => Checkpointed::Read {
      head: head.clone(),
      changes: Arc::new(ReadChanges {
        from: read.base,
        diff: Arc::new(Diff::parse(&read.text)?),
        key: format!("{base}..{head}").into(),
      }),
    },
    RevisionChanges::TooLarge(size) => Checkpointed::TooLarge {
      head: head.clone(),
      size,
    },
  })
}

/// What the worktree of `run` holds beyond its commit; `ended` when the run ended, so a missing
/// worktree was removed rather than not created yet.
pub fn not_checkpointed_in(
  repo_root: &Path,
  programs: &Programs,
  (run, ended): (RunId, bool),
) -> Result<NotCheckpointed, ChangesError> {
  let worktree = run_worktree(repo_root, run);
  if !worktree.is_dir() {
    return Ok(if ended {
      NotCheckpointed::Removed
    } else {
      NotCheckpointed::Clean
    });
  }
  let read = read_worktree_changes(&programs.runner(), Path::new(GIT), &worktree)?;
  Ok(match read.read {
    WorktreeRead::Clean => NotCheckpointed::Clean,
    WorktreeRead::TooLarge(size) => NotCheckpointed::TooLarge { from: read.head, size },
    WorktreeRead::Read(text) => {
      // The same changes read again keep their key, so what was highlighted stays.
      let mut hasher = DefaultHasher::new();
      text.hash(&mut hasher);
      NotCheckpointed::Read(Arc::new(ReadChanges {
        key: format!("{}:{:016x}", read.head, hasher.finish()).into(),
        diff: Arc::new(Diff::parse(&text)?),
        from: read.head,
      }))
    }
  })
}
