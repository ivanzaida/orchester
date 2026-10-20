//! A run's changes as the task page's Changes tab shows them: its diff from where its own work starts (its base
//! revision, or the newer commit of its base branch its branch took in, [`run_changes_in`]) to its latest
//! checkpoint, read with git once per pair of commits and stored, and what its worktree
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
  integration::{
    RunWork, WORKTREES_DIR,
    local::{RebaseStore, RunRebase},
    own_base,
  },
  revision::Revision,
  run::{Run, RunStatus},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use crate::messages::Message;

mod count;
mod follow;

pub use self::{
  count::listed_files,
  follow::{Follower, Refresh},
};

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

/// Where a run is, for what its missing worktree means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RunStage {
  /// Not started: its worktree is made when it starts.
  NotStarted,
  /// Started and not ended, interrupted or not: its worktree should be there.
  Started,
  /// Ended: its worktree may have been removed.
  Ended,
}

impl RunStage {
  pub const fn of(status: &RunStatus) -> Self {
    match status {
      RunStatus::Pending => Self::NotStarted,
      _ if status.is_terminal() => Self::Ended,
      _ => Self::Started,
    }
  }
}

#[derive(Debug, Error)]
pub enum ChangesError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Git(#[from] GitDiffError),
  #[error("git's diff is not a unified diff: {0}")]
  Diff(#[from] DiffParseError),
  #[error("the worktree {0} of a run that has not ended is missing")]
  WorktreeMissing(PathBuf),
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
      Self::WorktreeMissing(_) => Message::new("runs.changes.failure.worktreeMissing"),
    }
  }
}

/// What a run's checkpointed changes are read between: its work, from the revision it started from, and its latest
/// checkpoint (`None` before its first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRange {
  pub run: RunId,
  pub work: RunWork,
  pub head: Option<Revision>,
}

impl RunRange {
  pub fn of(run: &Run) -> Self {
    Self {
      run: run.id(),
      work: RunWork::of(run),
      head: run.last_checkpoint().map(|checkpoint| checkpoint.revision().clone()),
    }
  }
}

/// The run's own changes up to its latest checkpoint: from where its work starts, above the commits of its base
/// branch its branch took in since it started (ORC-60, [`own_base`]), else from the revision it started from.
pub fn run_changes_in(repo_root: &Path, programs: &Programs, range: &RunRange) -> Result<Checkpointed, ChangesError> {
  let Some(head) = &range.head else {
    return Ok(Checkpointed::NoCheckpoint);
  };
  let db = ProjectDb::open(repo_root)?;
  let checkpoints = db.checkpoint_revisions(range.run)?;
  let proven = db.run_rebase(range.run)?;
  let base = match own_base(
    &programs.runner(),
    (Path::new(GIT), repo_root),
    (&range.work, head),
    (&checkpoints, proven.as_ref().map(RunRebase::base)),
  ) {
    Ok(base) => base,
    // The diff read next tells what git cannot read; the run's own start is the safe one meanwhile.
    Err(error) => {
      tracing::warn!(%error, run = %range.run, "cannot tell where the run's own work starts; reading from its start");
      range.work.base.clone()
    }
  };
  checkpointed_in(repo_root, programs, &base, Some(head))
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

/// What the worktree of `run` holds beyond its commit. A missing worktree was not made yet before the run
/// started, was removed after it ended, and is an error in between.
pub fn not_checkpointed_in(
  repo_root: &Path,
  programs: &Programs,
  (run, stage): (RunId, RunStage),
) -> Result<NotCheckpointed, ChangesError> {
  let worktree = run_worktree(repo_root, run);
  if !worktree.is_dir() {
    return match stage {
      RunStage::NotStarted => Ok(NotCheckpointed::Clean),
      RunStage::Ended => Ok(NotCheckpointed::Removed),
      RunStage::Started => Err(ChangesError::WorktreeMissing(worktree)),
    };
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
