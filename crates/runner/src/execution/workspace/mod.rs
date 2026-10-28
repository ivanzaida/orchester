//! Where a step execution works (`docs/architecture/storage.md`, "Agent worktrees").
//!
//! Parallel steps writing to one worktree would overwrite each other's files, and a merge into a
//! worktree an agent is working in would change files under it. So:
//!
//! - A step that starts alone — no other step of the run is running and no other one is ready — works in the run's
//!   worktree, on the run's branch, as a sequential pipeline always has. Nothing else can start while it runs: only a
//!   passing step makes another one ready.
//! - A step that starts beside others, write or read-only alike, works in a worktree of its own on a branch off the
//!   run's branch as it is when the step starts, which holds every step it depends on. When it passes, its branch is
//!   merged back into the run's branch in the run's worktree, one merge at a time, and its worktree and branch are
//!   removed. A conflict fails the step with the files named; its branch and worktree stay for inspection.
//!
//! Which one an execution uses is recorded by the worktree itself: an execution with a worktree of
//! its own works there, also when it continues after a pause, a limit or an interruption. A run that borrows warm
//! workspaces (ORC-114) has no worktrees of its own: its steps beside others borrow a workspace each ([`borrowed`]).

mod borrowed;

use std::path::{Path, PathBuf};

use orchester_core::{
  id::StepExecutionId,
  run::{AcceptRefusal, Run, StepExecution},
};

use super::{checkpoint, prepare::RunFolder, run::RunTask};
use crate::{
  git::Git,
  worktree::{WorktreeError, Worktrees},
};

/// How an execution starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Start {
  /// New, with no other step of the run running or ready.
  Alone,
  /// New, while other steps of the run run or are ready to.
  BesideOthers,
  /// It started earlier and continues where it worked before.
  Continues,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Workspace {
  /// The run's worktree, on the run's branch: the step runs alone.
  Run(PathBuf),
  /// A worktree of the step's own, on `branch` off the run's branch; `borrowed` when it is a warm workspace the step
  /// gives back once its branch is merged, rather than a worktree made for it.
  Own {
    path: PathBuf,
    branch: String,
    owner: StepExecutionId,
    borrowed: bool,
  },
}

impl RunTask {
  /// Where `execution` works when it starts as `start` says; `Ok(None)` while it waits for a free workspace. Why it
  /// cannot work anywhere, otherwise.
  pub(super) fn workspace(&mut self, execution: StepExecutionId, start: Start) -> Result<Option<Workspace>, String> {
    match self.folder {
      RunFolder::Own(_) => Workspace::of(&self.root, &self.run, self.worktree(), execution, start)
        .map(Some)
        .map_err(|error| error.to_string()),
      RunFolder::Borrowed => self.borrowed_workspace(execution, start),
    }
  }

  /// [`Workspace::accepted_work`] for the run, borrowing or not.
  pub(super) fn accepted_work(
    &self,
    accepted: &StepExecution,
  ) -> Result<Result<Workspace, AcceptRefusal>, WorktreeError> {
    match self.folder {
      RunFolder::Own(_) => Workspace::accepted_work(&self.root, (&self.run, self.worktree()), accepted),
      RunFolder::Borrowed => self.borrowed_accepted_work(accepted),
    }
  }
}

impl Workspace {
  pub(super) fn path(&self) -> &Path {
    match self {
      Self::Run(path) | Self::Own { path, .. } => path,
    }
  }

  /// Where `execution` of `run` works when it starts as `start` says. A new execution beside
  /// others gets a worktree of its own, created now from the head of the run's worktree
  /// `run_worktree`.
  pub(super) fn of(
    root: &Path,
    run: &Run,
    run_worktree: &Path,
    execution: StepExecutionId,
    start: Start,
  ) -> Result<Self, WorktreeError> {
    let worktrees = Worktrees::new(root)?;
    let mut previous = run.recovered_execution(execution);
    while let Some(earlier) = previous {
      let path = worktrees.step_path(earlier);
      if path.is_dir() {
        return Ok(Self::Own {
          path,
          branch: branch(run, earlier),
          owner: earlier,
          borrowed: false,
        });
      }
      previous = run.recovered_execution(earlier);
    }
    let path = worktrees.step_path(execution);
    let branch = branch(run, execution);
    if path.is_dir() {
      return Ok(Self::Own {
        path,
        branch,
        owner: execution,
        borrowed: false,
      });
    }
    if start != Start::BesideOthers {
      return Ok(Self::Run(run_worktree.to_path_buf()));
    }
    let base = checkpoint::head(run_worktree)?;
    worktrees.create_step(execution, &branch, base.as_str())?;
    Ok(Self::Own {
      path,
      branch,
      owner: execution,
      borrowed: false,
    })
  }

  /// Where the work of `accepted`, an execution a person accepted, is — its own worktree if it had one, which is
  /// merged when the step passes, else the run's, where its work was committed already — when that work is what its
  /// result was for: it builds on the revision the execution was judged on (later commits on top of it pass with it),
  /// and nothing uncommitted lies beside it that passing would commit as the step's. Why not, otherwise; the outer
  /// error when git or the worktrees cannot be read.
  pub(super) fn accepted_work(
    root: &Path,
    (run, run_worktree): (&Run, &Path),
    accepted: &StepExecution,
  ) -> Result<Result<Self, AcceptRefusal>, WorktreeError> {
    let Some(judged) = accepted.judged() else {
      return Ok(Err(AcceptRefusal::NotJudged));
    };
    let workspace = match Self::of_accepted(root, run, run_worktree, accepted.id())? {
      Ok(workspace) => workspace,
      Err(refusal) => return Ok(Err(refusal)),
    };
    let git = Git::new(workspace.path());
    if !git.answer(&["merge-base", "--is-ancestor", judged.as_str(), "HEAD"])? {
      return Ok(Err(AcceptRefusal::NotBuiltOn {
        revision: judged.as_str().chars().take(12).collect(),
      }));
    }
    if !git
      .stdout(&["status", "--porcelain", "--untracked-files=all"])?
      .trim()
      .is_empty()
    {
      return Ok(Err(AcceptRefusal::Uncommitted));
    }
    Ok(Ok(workspace))
  }

  /// Where the work of `accepted`, an execution a person accepted, is: its own worktree if it had one, which is
  /// merged when the step passes; else the run's, where its work was committed already. Refused when it had one that
  /// was removed while its branch remains: the run's worktree does not hold that work.
  fn of_accepted(
    root: &Path,
    run: &Run,
    run_worktree: &Path,
    accepted: StepExecutionId,
  ) -> Result<Result<Self, AcceptRefusal>, WorktreeError> {
    let path = Worktrees::new(root)?.step_path(accepted);
    let branch = branch(run, accepted);
    if path.is_dir() {
      return Ok(Ok(Self::Own {
        path,
        branch,
        owner: accepted,
        borrowed: false,
      }));
    }
    if Git::new(root).answer(&["show-ref", "--verify", "--quiet", &format!("refs/heads/{branch}")])? {
      return Ok(Err(AcceptRefusal::WorktreeRemoved { branch }));
    }
    Ok(Ok(Self::Run(run_worktree.to_path_buf())))
  }
}

/// The branch of a step working beside others: `orchester/orc-12-run-1--review-2` for step
/// `review` in attempt 2.
fn branch(run: &Run, execution: StepExecutionId) -> String {
  let step = run
    .execution(execution)
    .map_or_else(String::new, |execution| execution.step().to_string());
  let attempt = run
    .attempts()
    .iter()
    .find(|attempt| attempt.execution(execution).is_some())
    .map_or(0, |attempt| attempt.number().get());
  format!("{}--{step}-{attempt}", run.branch())
}
