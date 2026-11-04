//! What a run's worktree holds beyond its checked-out commit, read here where the worktree is, for the task page's
//! Changes and `get_changes` ([`orchester_ipc::Command::ReadWorktreeChanges`]): the desktop reads no run's worktree
//! itself (`docs/architecture/remote-runners.md`, rule 5). Only read-only git commands run, without optional locks,
//! beside an agent at work.

use std::{path::Path, time::Duration};

use orchester_core::{
  command::CommandError,
  diff::{
    Changes, Diff,
    git::{GitDiffError, WorktreeRead, read_worktree_changes},
  },
  id::RunId,
};
use orchester_exec::ProcessRunner;
use orchester_ipc::{Reply, WorktreeChangesRead, WorktreeReadFailure, frame::MAX_FRAME_LEN};

use crate::{execution::Scheduler, execution_root::ExecutionRoot};

/// A git that does not answer within this time counts as failed, as the desktop's own reads did.
const GIT_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest diff a reply carries: a frame holds at most [`MAX_FRAME_LEN`], and JSON escapes some characters of
/// the text with more than one. A longer one is told as too large, with its size, as a diff with too many lines is.
const MAX_DIFF_LEN: usize = MAX_FRAME_LEN as usize / 4;

impl Scheduler {
  /// The answer to [`orchester_ipc::Command::ReadWorktreeChanges`] for `run`; git runs off the runner's tasks.
  pub async fn worktree_changes(self, run: RunId) -> Reply {
    let execution = ExecutionRoot::local(&self.inner.project_root);
    let read = tokio::task::spawn_blocking(move || read(&execution, run))
      .await
      .unwrap_or_else(|error| {
        tracing::error!(%error, %run, "reading the run's worktree ended unexpectedly");
        WorktreeChangesRead::Failed(WorktreeReadFailure::Git {
          detail: "the read ended unexpectedly".to_owned(),
        })
      });
    Reply::WorktreeChanges { run, read }
  }
}

/// What the worktree of `run` in `execution` holds beyond its commit.
fn read(execution: &ExecutionRoot, run: RunId) -> WorktreeChangesRead {
  let worktree = execution.run_worktree(run);
  if !worktree.is_dir() {
    return WorktreeChangesRead::NoWorktree;
  }
  let read = match read_worktree_changes(&ProcessRunner::new(GIT_TIMEOUT), Path::new("git"), &worktree) {
    Ok(read) => read,
    Err(GitDiffError::Run(CommandError::NotFound)) => {
      return WorktreeChangesRead::Failed(WorktreeReadFailure::GitMissing);
    }
    Err(error) => return failed(run, &error.to_string()),
  };
  let head = read.head;
  match read.read {
    WorktreeRead::Clean => WorktreeChangesRead::Clean,
    WorktreeRead::TooLarge(size) => WorktreeChangesRead::TooLarge { head, size },
    WorktreeRead::Read(diff) if diff.len() > MAX_DIFF_LEN => match Diff::parse(&diff) {
      Ok(parsed) => WorktreeChangesRead::TooLarge {
        head,
        size: Changes {
          files: u32::try_from(parsed.files().len()).unwrap_or(u32::MAX),
          additions: parsed.additions(),
          deletions: parsed.deletions(),
        },
      },
      Err(error) => failed(run, &format!("git's diff is not a unified diff: {error}")),
    },
    WorktreeRead::Read(diff) => WorktreeChangesRead::Read { head, diff },
  }
}

fn failed(run: RunId, detail: &str) -> WorktreeChangesRead {
  tracing::warn!(%run, detail, "cannot read the run's worktree");
  WorktreeChangesRead::Failed(WorktreeReadFailure::Git {
    detail: detail.to_owned(),
  })
}
