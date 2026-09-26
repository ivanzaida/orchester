//! Git worktrees for runs under `<repo>/.orchester/worktrees/<run-id>/`, driven through the
//! `git` CLI. Creating one adds a new branch without touching the user's checkout; removing one
//! refuses to discard uncommitted work unless forced, and always keeps the run's branch. A step
//! that runs beside other steps of its run works in a worktree of its own,
//! `<repo>/.orchester/worktrees/<step-execution-id>/`, on a branch off the run's branch.

use std::{
  ffi::OsStr,
  io,
  path::{Path, PathBuf},
};

use orchester_core::id::{RunId, StepExecutionId};
use thiserror::Error;

use crate::git::Git;

const WORKTREES_DIR: &str = ".orchester/worktrees";

#[derive(Debug, Error)]
pub enum WorktreeError {
  #[error("cannot resolve repository path {path}: {source}")]
  RepositoryPath { path: PathBuf, source: io::Error },
  #[error("cannot run git: {0}")]
  GitUnavailable(io::Error),
  #[error("`git {command}` failed ({code:?}): {stderr}")]
  Git {
    command: String,
    code: Option<i32>,
    stderr: String,
  },
  #[error("{0:?} is not a valid branch name")]
  InvalidBranch(String),
  #[error("branch {0} already exists")]
  BranchExists(String),
  #[error("{0:?} does not name a commit")]
  UnknownBase(String),
  #[error("git reported {0:?} where a commit id was expected")]
  UnexpectedRevision(String),
  #[error(".orchester/ is not ignored by git in {0}; a worktree there would show up in the user's checkout")]
  NotExcluded(PathBuf),
  #[error("worktree {0} already exists")]
  AlreadyExists(PathBuf),
  #[error("worktree {0} does not exist")]
  NotFound(PathBuf),
  #[error("worktree {path} has {changes} uncommitted change(s); commit them or remove it with force")]
  Dirty { path: PathBuf, changes: usize },
}

/// Whether removal may discard uncommitted changes in the worktree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoveMode {
  RefuseIfDirty,
  DiscardChanges,
}

/// A worktree created for a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
  path: PathBuf,
  branch: String,
  base_revision: String,
}

impl Worktree {
  pub fn path(&self) -> &Path {
    &self.path
  }

  pub fn branch(&self) -> &str {
    &self.branch
  }

  /// Full commit SHA the branch was created at.
  pub fn base_revision(&self) -> &str {
    &self.base_revision
  }
}

/// The run worktrees of one repository.
#[derive(Clone, Debug)]
pub struct Worktrees {
  repo_root: PathBuf,
}

impl Worktrees {
  pub fn new(repo_root: &Path) -> Result<Self, WorktreeError> {
    let repo_root = std::path::absolute(repo_root).map_err(|source| WorktreeError::RepositoryPath {
      path: repo_root.to_path_buf(),
      source,
    })?;
    Ok(Self { repo_root })
  }

  pub fn path(&self, run: RunId) -> PathBuf {
    self.repo_root.join(WORKTREES_DIR).join(run.to_string())
  }

  /// The worktree of step execution `execution`, when it runs beside other steps.
  pub fn step_path(&self, execution: StepExecutionId) -> PathBuf {
    self.repo_root.join(WORKTREES_DIR).join(execution.to_string())
  }

  /// Creates the run's worktree on a new `branch` starting at `base` (any commit-ish).
  pub fn create(&self, run: RunId, branch: &str, base: &str) -> Result<Worktree, WorktreeError> {
    self.create_at(self.path(run), branch, base)
  }

  /// Creates the worktree of step execution `execution` on a new `branch` starting at `base`.
  pub fn create_step(&self, execution: StepExecutionId, branch: &str, base: &str) -> Result<Worktree, WorktreeError> {
    self.create_at(self.step_path(execution), branch, base)
  }

  fn create_at(&self, path: PathBuf, branch: &str, base: &str) -> Result<Worktree, WorktreeError> {
    if path.exists() {
      return Err(WorktreeError::AlreadyExists(path));
    }
    let git = Git::new(&self.repo_root);
    if !git.succeeds(["check-ref-format", "--branch", branch])? {
      return Err(WorktreeError::InvalidBranch(branch.to_owned()));
    }
    if git.answer(["show-ref", "--verify", "--quiet", &format!("refs/heads/{branch}")])? {
      return Err(WorktreeError::BranchExists(branch.to_owned()));
    }
    let base_revision = git
      .stdout([
        "rev-parse",
        "--verify",
        "--quiet",
        "--end-of-options",
        &format!("{base}^{{commit}}"),
      ])
      .map_err(|error| match error {
        WorktreeError::Git { .. } => WorktreeError::UnknownBase(base.to_owned()),
        other => other,
      })?;
    if !git.answer(["check-ignore", "--quiet", WORKTREES_DIR])? {
      return Err(WorktreeError::NotExcluded(self.repo_root.clone()));
    }
    git.stdout([
      OsStr::new("worktree"),
      OsStr::new("add"),
      OsStr::new("-b"),
      OsStr::new(branch),
      path.as_os_str(),
      OsStr::new(&base_revision),
    ])?;
    Ok(Worktree {
      path,
      branch: branch.to_owned(),
      base_revision,
    })
  }

  /// Removes the run's worktree directory. The branch and its commits are kept.
  pub fn remove(&self, run: RunId, mode: RemoveMode) -> Result<(), WorktreeError> {
    self.remove_at(self.path(run), mode)
  }

  /// Removes the clean worktree of a step execution whose branch was merged into the run's
  /// branch, and that branch: its commits are part of the run's branch now.
  pub fn remove_merged_step(&self, execution: StepExecutionId, branch: &str) -> Result<(), WorktreeError> {
    self.remove_at(self.step_path(execution), RemoveMode::RefuseIfDirty)?;
    // `-d` would compare with the user's checked-out branch, not the run's branch it was merged
    // into; the caller merged it, so nothing is lost.
    Git::new(&self.repo_root).stdout(["branch", "--quiet", "-D", branch])?;
    Ok(())
  }

  fn remove_at(&self, path: PathBuf, mode: RemoveMode) -> Result<(), WorktreeError> {
    if !path.is_dir() {
      return Err(WorktreeError::NotFound(path));
    }
    let mut command = vec![OsStr::new("worktree"), OsStr::new("remove")];
    match mode {
      RemoveMode::RefuseIfDirty => {
        let status = Git::new(&path).stdout(["status", "--porcelain", "--untracked-files=all"])?;
        let changes = status.lines().count();
        if changes > 0 {
          return Err(WorktreeError::Dirty { path, changes });
        }
      }
      RemoveMode::DiscardChanges => command.push(OsStr::new("--force")),
    }
    command.push(path.as_os_str());
    Git::new(&self.repo_root).stdout(command)?;
    Ok(())
  }
}
