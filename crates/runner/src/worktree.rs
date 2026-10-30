//! Git worktrees for runs under `<repo>/.orchester/worktrees/<run-id>/`, driven through the
//! `git` CLI. Creating one adds a new branch without touching the user's checkout; removing one
//! refuses to discard uncommitted work unless forced, and always keeps the run's branch. A step
//! that runs beside other steps of its run works in a worktree of its own,
//! `<repo>/.orchester/worktrees/<step-execution-id>/`, on a branch off the run's branch, which
//! starts with the run's build output ([`warm_start`]).

mod names;
pub mod warm_start;

use std::{
  fs, io, iter,
  path::{Path, PathBuf},
};

use orchester_core::{
  command::CommandError,
  id::{RunId, StepExecutionId},
  integration::{ListedWorktree, parse_worktree_list},
  run::{GitMessage, WorktreeFailure},
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
  #[error("branch {0} and each of its numbered names up to -{MAX_SUFFIX} are taken")]
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
  #[error("{0} is not a worktree of the repository on a branch")]
  NotAWorktree(PathBuf),
  #[error("worktree {path} has {changes} uncommitted change(s); commit them or remove it with force")]
  Dirty { path: PathBuf, changes: usize },
  #[error("git no longer lists worktree {path}, but its folder cannot be deleted: {source}")]
  NotDeleted { path: PathBuf, source: io::Error },
}

impl WorktreeError {
  /// Why a run whose worktree could not be created for this error never started, in words a person acts on. Only
  /// git's own refusal keeps its message, as far as [`GitMessage`] keeps it; the runner's log has the rest.
  pub fn failure(&self) -> WorktreeFailure {
    match self {
      Self::NotExcluded(_) => WorktreeFailure::NotIgnored,
      Self::NoFreeBranch(_) | Self::BranchExists(_) => WorktreeFailure::NoFreeBranch,
      Self::GitUnavailable(_) => WorktreeFailure::GitUnavailable,
      Self::UnknownBase(_) => WorktreeFailure::UnknownBase,
      Self::InvalidBranch(_) => WorktreeFailure::InvalidBranch,
      Self::AlreadyExists(_) | Self::NotFound(_) | Self::NotAWorktree(_) => WorktreeFailure::FolderInTheWay,
      Self::Git { stderr, .. } => WorktreeFailure::Git(GitMessage::new(stderr)),
      Self::RepositoryPath { .. } | Self::UnexpectedRevision(_) | Self::Dirty { .. } | Self::NotDeleted { .. } => {
        WorktreeFailure::Unexpected
      }
    }
  }
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
  /// the first free one of `branch-2`, `branch-3`, … when that name is taken (an earlier task with
  /// the same key, a removed worktree's, one a person made). A branch of the name takes it, and so
  /// does one of its step branches (`<name>--…`). An existing branch may hold someone's work, so it
  /// is never reused; [`Worktree::branch`] is the one it is on.
  pub fn create(&self, run: RunId, branch: &str, base: &str) -> Result<Worktree, WorktreeError> {
    let candidates = (2..=MAX_SUFFIX).map(|suffix| format!("{branch}-{suffix}"));
    self
      .create_at(
        &run.to_string(),
        branch,
        iter::once(branch.to_owned()).chain(candidates),
        base,
      )?
      .ok_or_else(|| WorktreeError::NoFreeBranch(branch.to_owned()))
  }

  /// Creates the worktree of step execution `execution` on a new `branch` starting at `base`. Its
  /// name follows from the run's branch, so it is that one or none.
  pub fn create_step(&self, execution: StepExecutionId, branch: &str, base: &str) -> Result<Worktree, WorktreeError> {
    self
      .create_at(&execution.to_string(), branch, iter::once(branch.to_owned()), base)?
      .ok_or_else(|| WorktreeError::BranchExists(branch.to_owned()))
  }

  /// Creates a run's branch at `base` without a worktree, for a run that borrows a workspace (ORC-114): `branch`, or
  /// the first free numbered name, as [`Worktrees::create`] picks it. Returns the branch made.
  pub fn create_branch(&self, branch: &str, base: &str) -> Result<String, WorktreeError> {
    let git = Git::new(&self.repo_root);
    if !git.succeeds(&["check-ref-format", "--branch", branch])? {
      return Err(WorktreeError::InvalidBranch(branch.to_owned()));
    }
    let base_revision = self.base_revision(&git, base)?;
    let taken = names::taken(&git, branch)?;
    let candidates = iter::once(branch.to_owned()).chain((2..=MAX_SUFFIX).map(|suffix| format!("{branch}-{suffix}")));
    for candidate in candidates.filter(|candidate| !names::is_taken(&taken, candidate)) {
      match git.stdout(&["branch", "--no-track", &candidate, &base_revision]) {
        Ok(_) => return Ok(candidate),
        Err(error) if names::taken_meanwhile(&candidate, &error) => {}
        Err(error) => return Err(error),
      }
    }
    Err(WorktreeError::NoFreeBranch(branch.to_owned()))
  }

  /// The branch the run's worktree has checked out. Its folder counts only when git lists it as a
  /// worktree of the repository: in any other folder git would answer for the checkout around it.
  pub fn branch_of(&self, run: RunId) -> Result<String, WorktreeError> {
    let path = self.path(run);
    if !path.is_dir() {
      return Err(WorktreeError::NotFound(path));
    }
    let here = fs::canonicalize(&path).map_err(|source| WorktreeError::RepositoryPath {
      path: path.clone(),
      source,
    })?;
    // A listed folder that cannot be resolved is gone, so it is not this one.
    let listed = self
      .listed()?
      .into_iter()
      .find(|listed| fs::canonicalize(&listed.path).is_ok_and(|there| there == here));
    match listed.and_then(|listed| listed.branch) {
      Some(branch) => Ok(branch.as_str().to_owned()),
      None => Err(WorktreeError::NotAWorktree(path)),
    }
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

  /// The repository's worktrees as git lists them.
  fn listed(&self) -> Result<Vec<ListedWorktree>, WorktreeError> {
    let listed = Git::new(&self.repo_root).stdout(&["worktree", "list", "--porcelain", "-z"])?;
    Ok(parse_worktree_list(&listed))
  }

  /// Creates the worktree folder `name` on the first of `candidates`, `branch` and its numbered
  /// names, that is not taken; `None` when every one is.
  fn create_at(
    &self,
    name: &str,
    branch: &str,
    candidates: impl Iterator<Item = String>,
    base: &str,
  ) -> Result<Option<Worktree>, WorktreeError> {
    let path = self.at(name);
    if path.exists() {
      return Err(WorktreeError::AlreadyExists(path));
    }
    let git = Git::new(&self.repo_root);
    // A numbered name of a valid name is valid too.
    if !git.succeeds(&["check-ref-format", "--branch", branch])? {
      return Err(WorktreeError::InvalidBranch(branch.to_owned()));
    }
    let base_revision = self.base_revision(&git, base)?;
    let taken = names::taken(&git, branch)?;
    for candidate in candidates.filter(|candidate| !names::is_taken(&taken, candidate)) {
      if self.add(&git, name, &candidate, &base_revision)? {
        return Ok(Some(Worktree {
          path,
          branch: candidate,
          base_revision,
        }));
      }
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

  /// Creates `branch` at `base_revision` and checks it out in the worktree folder `name`; `false`
  /// when someone took the name meanwhile ([`names::taken_meanwhile`]). `git branch` creates it or
  /// fails as one step, so such a branch is never taken over.
  fn add(&self, git: &Git<'_>, name: &str, branch: &str, base_revision: &str) -> Result<bool, WorktreeError> {
    if let Err(error) = git.stdout(&["branch", "--no-track", branch, base_revision]) {
      return if names::taken_meanwhile(branch, &error) {
        Ok(false)
      } else {
        Err(error)
      };
    }
    if let Err(error) = git.stdout(&["worktree", "add", &Self::relative(name), branch]) {
      self.discard_added(git, name, branch, base_revision);
      return Err(error);
    }
    Ok(true)
  }

  /// Undoes what a `git worktree add` that failed left. Git keeps the worktree when only its
  /// `post-checkout` hook failed: it is fresh and holds nothing of anyone's, so it is removed with
  /// force. The new branch is deleted only once no worktree has it checked out, and only while it
  /// is still at `base_revision`, so no work is lost and it does not take the name of the next try.
  /// What cannot be undone is left and logged; the creation fails for git's first error either way.
  fn discard_added(&self, git: &Git<'_>, name: &str, branch: &str, base_revision: &str) {
    let relative = Self::relative(name);
    if self.at(name).exists()
      && let Err(error) = git.stdout(&["worktree", "remove", "--force", "--force", &relative])
    {
      tracing::warn!(%relative, %error, "the worktree git kept after failing to add it is left");
    }
    let checked_out = self.listed().map(|listed| {
      listed.iter().any(|worktree| {
        worktree
          .branch
          .as_ref()
          .is_some_and(|checked| checked.as_str() == branch)
      })
    });
    match checked_out {
      Ok(false) => {
        if let Err(error) = git.stdout(&["update-ref", "-d", &format!("refs/heads/{branch}"), base_revision]) {
          tracing::warn!(%branch, %error, "the new branch of a worktree that could not be added is left");
        }
      }
      Ok(true) => tracing::warn!(%branch, "a worktree has the new branch checked out, so the branch is left"),
      Err(error) => tracing::warn!(%branch, %error, "cannot tell whether a worktree has the new branch; it is left"),
    }
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
    let removed = Git::new(&self.repo_root).stdout(&command).map(drop);
    // Git can forget the worktree and stop with its folder half deleted ("Directory not empty"): what is left of it,
    // ignored build output and `.tmp/` included, goes as a folder once git no longer lists it.
    if !path.exists() || self.lists(name)? {
      return removed;
    }
    fs::remove_dir_all(&path).map_err(|source| WorktreeError::NotDeleted { path, source })
  }

  /// Whether git lists a worktree in the folder `name`.
  fn lists(&self, name: &str) -> Result<bool, WorktreeError> {
    Ok(
      self
        .listed()?
        .iter()
        .any(|listed| Path::new(&listed.path).file_name().is_some_and(|listed| listed == name)),
    )
  }
}
