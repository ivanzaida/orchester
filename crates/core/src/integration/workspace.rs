//! Git in the project's warm workspaces (ORC-114), for the runner and the app's merge queue alike. A workspace is a
//! git worktree at `.orchester/workspaces/<n>` that is never left on a branch while idle: giving it back detaches its
//! HEAD, so any run's branch can be checked out in any idle workspace. Switching it to another run checks out that
//! run's branch, which changes only the files that differ, and then cleans every git-ignored file except the warm
//! folders kept between runs. Uncommitted changes are never thrown away unseen: a workspace that holds some cannot be
//! switched, and a person keeps them in a branch or discards them.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use thiserror::Error;

use super::{GitError, cleanup::parse_worktree_list, git::Git};
use crate::{
  command::CommandRunner,
  run::BranchName,
  workspaces::{Cleaning, WORKSPACES_DIR, WarmFolder, WorkspaceNumber, cleaning},
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum WorkspaceGitError {
  #[error(transparent)]
  Git(#[from] GitError),
  /// `.orchester/` is not ignored by git: a workspace there would show in the person's checkout.
  #[error(".orchester/ is not ignored by git in {0}")]
  NotIgnored(PathBuf),
  #[error("{0} is in the way of a workspace: it is no worktree of the repository")]
  InTheWay(PathBuf),
  /// The workspace holds these uncommitted files; switching it would lose them.
  #[error("the workspace holds {} uncommitted file(s)", .0.len())]
  Uncommitted(Vec<String>),
  #[error("cannot clean {path}: {detail}")]
  Io { path: PathBuf, detail: String },
}

/// Deletes the ignored entry `entry` of the workspace at `root` (a folder ends in `/`) unless `folders` keep it,
/// going into a folder that holds kept ones deeper down.
fn remove_unkept(root: &Path, entry: &str, folders: &[WarmFolder]) -> io::Result<()> {
  let path = root.join(entry.trim_end_matches('/'));
  match cleaning(folders, entry) {
    Cleaning::Keep => Ok(()),
    Cleaning::Remove if entry.ends_with('/') => match fs::remove_dir_all(&path) {
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
      other => other,
    },
    Cleaning::Remove => match fs::remove_file(&path) {
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
      other => other,
    },
    Cleaning::Descend => {
      for child in fs::read_dir(&path)? {
        let child = child?;
        let name = child.file_name();
        let Some(name) = name.to_str() else {
          // A name git could not have listed as text is left as it is.
          continue;
        };
        let inner = if child.file_type()?.is_dir() {
          format!("{entry}{name}/")
        } else {
          format!("{entry}{name}")
        };
        remove_unkept(root, &inner, folders)?;
      }
      Ok(())
    }
  }
}

/// What a workspace is switched to.
#[derive(Clone, Copy, Debug)]
pub enum SwitchTo<'a> {
  /// A run's branch, or one of its steps'.
  Branch(&'a BranchName),
  /// A commit, detached: the base branch for the warm-up.
  Commit(&'a str),
}

/// Git for the workspaces of the repository at `repo`.
pub struct Workspaces<'a, R: CommandRunner> {
  runner: &'a R,
  program: &'a Path,
  repo: &'a Path,
}

impl<'a, R: CommandRunner> Workspaces<'a, R> {
  pub const fn new(runner: &'a R, program: &'a Path, repo: &'a Path) -> Self {
    Self { runner, program, repo }
  }

  fn git(&self, dir: &'a Path) -> Git<'a, R> {
    Git::new(self.runner, self.program, dir)
  }

  /// The folder of workspace `number`, made first as a worktree with a detached HEAD at `base` when it is not there.
  pub fn prepare(&self, number: WorkspaceNumber, base: &str) -> Result<PathBuf, WorkspaceGitError> {
    let path = number.path(self.repo);
    if path.join(".git").exists() {
      return Ok(path);
    }
    if path.exists() {
      return Err(WorkspaceGitError::InTheWay(path));
    }
    let git = self.git(self.repo);
    if !git.succeeds(&["check-ignore", "--quiet", WORKSPACES_DIR])? {
      return Err(WorkspaceGitError::NotIgnored(self.repo.to_path_buf()));
    }
    // A folder deleted by hand is still registered until pruned, which keeps git from adding it again.
    git.stdout(&["worktree", "prune"])?;
    git.stdout(&["worktree", "add", "--quiet", "--detach", &number.relative(), base])?;
    Ok(path)
  }

  /// The files with uncommitted changes in the workspace at `path`, untracked ones included.
  pub fn uncommitted(&self, path: &Path) -> Result<Vec<String>, WorkspaceGitError> {
    Ok(Git::new(self.runner, self.program, path).uncommitted()?)
  }

  /// Switches the workspace at `path` to `to`, then cleans every ignored file but the folders `folders` keep. With
  /// `carry` — the run that worked there last goes on there — its uncommitted changes go along and nothing is
  /// cleaned; otherwise uncommitted changes refuse the switch ([`WorkspaceGitError::Uncommitted`]).
  pub fn switch(
    &self,
    path: &Path,
    to: SwitchTo<'_>,
    (folders, carry): (&[WarmFolder], bool),
  ) -> Result<(), WorkspaceGitError> {
    let git = Git::new(self.runner, self.program, path);
    let uncommitted = git.uncommitted()?;
    if !uncommitted.is_empty() && !carry {
      return Err(WorkspaceGitError::Uncommitted(uncommitted));
    }
    match to {
      SwitchTo::Branch(branch) => {
        if git.checked_out_branch()?.as_ref() != Some(branch) {
          self.free_branch(branch, path)?;
          git.stdout(&["checkout", "--quiet", branch.as_str()])?;
        }
      }
      SwitchTo::Commit(commit) => {
        git.stdout(&["checkout", "--quiet", "--detach", commit])?;
      }
    }
    if !carry {
      self.clean_ignored(path, folders)?;
    }
    Ok(())
  }

  /// Deletes every git-ignored file and folder of the workspace at `path` but the folders of `folders` kept between
  /// runs. `git clean` removes an ignored folder whole whatever a pathspec excludes, so git only lists the ignored
  /// entries, and the folders that hold kept ones deeper down are gone through here.
  fn clean_ignored(&self, path: &Path, folders: &[WarmFolder]) -> Result<(), WorkspaceGitError> {
    let output = Git::new(self.runner, self.program, path).output(&[
      "ls-files",
      "--others",
      "--ignored",
      "--exclude-standard",
      "--directory",
      "-z",
    ])?;
    if !output.success() {
      return Err(GitError::Failed(super::git::message(&output)).into());
    }
    for entry in output.stdout.split('\0').filter(|entry| !entry.is_empty()) {
      remove_unkept(path, entry, folders).map_err(|error| WorkspaceGitError::Io {
        path: path.join(entry),
        detail: error.to_string(),
      })?;
    }
    Ok(())
  }

  /// Detaches `branch` in an idle workspace other than `path` that still has it checked out — one a runner that
  /// stopped did not give back — so it can be checked out here. Its files stay as they are.
  fn free_branch(&self, branch: &BranchName, path: &Path) -> Result<(), WorkspaceGitError> {
    let listed = self.git(self.repo).stdout(&["worktree", "list", "--porcelain", "-z"])?;
    let here = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    for other in parse_worktree_list(&listed) {
      let other_path = PathBuf::from(&other.path);
      let in_workspaces = other_path
        .parent()
        .is_some_and(|parent| parent.ends_with(Path::new(WORKSPACES_DIR)));
      let elsewhere = other_path.canonicalize().map_or(true, |there| there != here);
      if other.branch.as_ref() == Some(branch) && in_workspaces && elsewhere {
        Git::new(self.runner, self.program, &other_path).stdout(&["checkout", "--quiet", "--detach"])?;
      }
    }
    Ok(())
  }

  /// Commits every change in the workspace at `path` with `message`; the new commit, or `None` when nothing changed.
  pub fn commit(&self, path: &Path, message: &str) -> Result<Option<String>, WorkspaceGitError> {
    let git = Git::new(self.runner, self.program, path);
    if git.uncommitted()?.is_empty() {
      return Ok(None);
    }
    git.stdout(&["add", "--all"])?;
    git.stdout(&["commit", "--quiet", "-m", message])?;
    Ok(Some(git.stdout(&["rev-parse", "HEAD"])?))
  }

  /// Gives the workspace at `path` back: its HEAD is detached where it is, so its branch is free for any workspace
  /// and its files stay warm.
  pub fn detach(&self, path: &Path) -> Result<(), WorkspaceGitError> {
    Git::new(self.runner, self.program, path).stdout(&["checkout", "--quiet", "--detach"])?;
    Ok(())
  }

  /// "Keep them in a branch": commits the uncommitted changes of the workspace at `path` on the new branch `branch`
  /// made where its HEAD is, with `message`, and detaches it again.
  pub fn rescue(&self, path: &Path, branch: &BranchName, message: &str) -> Result<(), WorkspaceGitError> {
    let git = Git::new(self.runner, self.program, path);
    git.stdout(&["switch", "--quiet", "--create", branch.as_str()])?;
    self.commit(path, message)?;
    git.stdout(&["checkout", "--quiet", "--detach"])?;
    Ok(())
  }

  /// "Discard": throws away the uncommitted changes of the workspace at `path`, which a person saw and chose to lose.
  /// Ignored files, its warm folders among them, stay.
  pub fn discard(&self, path: &Path) -> Result<(), WorkspaceGitError> {
    let git = Git::new(self.runner, self.program, path);
    git.stdout(&["reset", "--quiet", "--hard"])?;
    git.stdout(&["clean", "-d", "-f", "--quiet"])?;
    Ok(())
  }

  /// Deletes every ignored file of the idle workspace at `path`, its warm folders too: the next run there builds
  /// from cold. Its committed work is on its branches.
  pub fn clean_warm(&self, path: &Path) -> Result<(), WorkspaceGitError> {
    Git::new(self.runner, self.program, path).stdout(&["clean", "-d", "-f", "-X", "--quiet"])?;
    Ok(())
  }

  /// Whether the work of `run` is still there for it to go on with: its own worktree, for a run made before
  /// workspaces; its branch, for one that `borrows` workspaces.
  pub fn run_work_present(&self, run: &crate::run::Run, borrows: bool) -> Result<bool, GitError> {
    let own = self.repo.join(super::WORKTREES_DIR).join(run.id().to_string());
    if own.is_dir() {
      return Ok(true);
    }
    Ok(
      borrows
        && self
          .git(self.repo)
          .commit(&format!("refs/heads/{}", run.branch()))?
          .is_some(),
    )
  }
}
