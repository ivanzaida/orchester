//! Git worktrees for runs under `<repo>/.orchester/worktrees/<run-id>/`, driven through the
//! `git` CLI. Creating one adds a new branch without touching the user's checkout; removing one
//! refuses to discard uncommitted work unless forced, and always keeps the run's branch. A step
//! that runs beside other steps of its run works in a worktree of its own,
//! `<repo>/.orchester/worktrees/<step-execution-id>/`, on a branch off the run's branch.

use std::{
  io, iter,
  path::{Path, PathBuf},
};

use orchester_core::{
  command::CommandError,
  id::{RunId, StepExecutionId},
};
use thiserror::Error;

use crate::git::Git;

const WORKTREES_DIR: &str = ".orchester/worktrees";
/// The last numbered name a run's branch may take when its own is taken. Leftovers never take that
/// many, so past it the run fails ([`WorktreeError::NoFreeBranch`]) instead of calling git on.
const MAX_SUFFIX: u32 = 100;

#[derive(Debug, Error)]
pub enum WorktreeError {
  #[error("cannot resolve repository path {path}: {source}")]
  RepositoryPath { path: PathBuf, source: io::Error },
  #[error("cannot run git: {0}")]
  GitUnavailable(CommandError),
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
  #[error("branch {0} and each of its numbered names up to -{MAX_SUFFIX} exist already")]
  NoFreeBranch(String),
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
    self.at(&run.to_string())
  }

  /// The worktree of step execution `execution`, when it runs beside other steps.
  pub fn step_path(&self, execution: StepExecutionId) -> PathBuf {
    self.at(&execution.to_string())
  }

  /// Creates the run's worktree on a new branch starting at `base` (any commit-ish): `branch`, or
  /// the first free one of `branch-2`, `branch-3`, … when a branch of that name exists already (an
  /// earlier task with the same key, a removed worktree's, one a person made). An existing branch
  /// may hold someone's work, so it is never reused; [`Worktree::branch`] is the one it is on.
  pub fn create(&self, run: RunId, branch: &str, base: &str) -> Result<Worktree, WorktreeError> {
    let candidates = (2..=MAX_SUFFIX).map(|suffix| format!("{branch}-{suffix}"));
    self
      .create_at(&run.to_string(), iter::once(branch.to_owned()).chain(candidates), base)?
      .ok_or_else(|| WorktreeError::NoFreeBranch(branch.to_owned()))
  }

  /// Creates the worktree of step execution `execution` on a new `branch` starting at `base`. Its
  /// name follows from the run's branch, so it is that one or none.
  pub fn create_step(&self, execution: StepExecutionId, branch: &str, base: &str) -> Result<Worktree, WorktreeError> {
    self
      .create_at(&execution.to_string(), iter::once(branch.to_owned()), base)?
      .ok_or_else(|| WorktreeError::BranchExists(branch.to_owned()))
  }

  /// The branch the run's worktree has checked out.
  pub fn branch_of(&self, run: RunId) -> Result<String, WorktreeError> {
    let path = self.path(run);
    if !path.is_dir() {
      return Err(WorktreeError::NotFound(path));
    }
    Git::new(&path).stdout(&["symbolic-ref", "--quiet", "--short", "HEAD"])
  }

  /// The worktree folder `name`.
  fn at(&self, name: &str) -> PathBuf {
    self.repo_root.join(WORKTREES_DIR).join(name)
  }

  /// The worktree folder `name` as git reads it in the repository's folder: relative, so a
  /// repository path that is not valid text still works.
  fn relative(name: &str) -> String {
    format!("{WORKTREES_DIR}/{name}")
  }

  /// Creates the worktree folder `name` on the first of `branches` that no branch has yet; `None`
  /// when every one is taken.
  fn create_at(
    &self,
    name: &str,
    branches: impl Iterator<Item = String>,
    base: &str,
  ) -> Result<Option<Worktree>, WorktreeError> {
    let path = self.at(name);
    if path.exists() {
      return Err(WorktreeError::AlreadyExists(path));
    }
    let git = Git::new(&self.repo_root);
    let base_revision = self.base_revision(&git, base)?;
    for branch in branches {
      if !git.succeeds(&["check-ref-format", "--branch", &branch])? {
        return Err(WorktreeError::InvalidBranch(branch));
      }
      if Self::branch_exists(&git, &branch)? || !Self::add(&git, name, &branch, &base_revision)? {
        continue;
      }
      return Ok(Some(Worktree {
        path,
        branch,
        base_revision,
      }));
    }
    Ok(None)
  }

  /// `base` as a full commit SHA, in a repository that ignores the worktrees.
  fn base_revision(&self, git: &Git<'_>, base: &str) -> Result<String, WorktreeError> {
    let base_revision = git
      .stdout(&[
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
    if !git.answer(&["check-ignore", "--quiet", WORKTREES_DIR])? {
      return Err(WorktreeError::NotExcluded(self.repo_root.clone()));
    }
    Ok(base_revision)
  }

  fn branch_exists(git: &Git<'_>, branch: &str) -> Result<bool, WorktreeError> {
    git.answer(&["show-ref", "--verify", "--quiet", &format!("refs/heads/{branch}")])
  }

  /// Creates `branch` at `base_revision` and checks it out in the worktree folder `name`; `false`
  /// when someone made a branch of that name meanwhile. `git branch` creates it or fails as one
  /// step, so such a branch is never taken over.
  fn add(git: &Git<'_>, name: &str, branch: &str, base_revision: &str) -> Result<bool, WorktreeError> {
    if let Err(error) = git.stdout(&["branch", "--no-track", branch, base_revision]) {
      return if Self::branch_exists(git, branch)? {
        Ok(false)
      } else {
        Err(error)
      };
    }
    if let Err(error) = git.stdout(&["worktree", "add", &Self::relative(name), branch]) {
      // The branch is new and holds nothing yet: deleting it only while it is still at
      // `base_revision` loses no work, and keeps it from taking the name of the next try.
      if let Err(cleanup) = git.stdout(&["update-ref", "-d", &format!("refs/heads/{branch}"), base_revision]) {
        tracing::warn!(%branch, %cleanup, "the new branch of a worktree that could not be added is left");
      }
      return Err(error);
    }
    Ok(true)
  }

  /// Removes the run's worktree directory. The branch and its commits are kept.
  pub fn remove(&self, run: RunId, mode: RemoveMode) -> Result<(), WorktreeError> {
    self.remove_at(&run.to_string(), mode)
  }

  /// Removes the clean worktree of a step execution whose branch was merged into the run's
  /// branch, and that branch: its commits are part of the run's branch now.
  pub fn remove_merged_step(&self, execution: StepExecutionId, branch: &str) -> Result<(), WorktreeError> {
    self.remove_at(&execution.to_string(), RemoveMode::RefuseIfDirty)?;
    // `-d` would compare with the user's checked-out branch, not the run's branch it was merged
    // into; the caller merged it, so nothing is lost.
    Git::new(&self.repo_root).stdout(&["branch", "--quiet", "-D", branch])?;
    Ok(())
  }

  fn remove_at(&self, name: &str, mode: RemoveMode) -> Result<(), WorktreeError> {
    let path = self.at(name);
    if !path.is_dir() {
      return Err(WorktreeError::NotFound(path));
    }
    let relative = Self::relative(name);
    let mut command = vec!["worktree", "remove"];
    match mode {
      RemoveMode::RefuseIfDirty => {
        let status = Git::new(&path).stdout(&["status", "--porcelain", "--untracked-files=all"])?;
        let changes = status.lines().count();
        if changes > 0 {
          return Err(WorktreeError::Dirty { path, changes });
        }
      }
      RemoveMode::DiscardChanges => command.push("--force"),
    }
    command.push(&relative);
    Git::new(&self.repo_root).stdout(&command)?;
    Ok(())
  }
}
