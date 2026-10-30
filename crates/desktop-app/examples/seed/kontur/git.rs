//! The QA repository's git, run through `orchester-exec` like every program Orchester starts: a new
//! repository with one commit on `main`, run worktrees in `.orchester/worktrees/<run>` on their run
//! branches as the runner adds them, checkpoint commits in them, and Merge into main.

use std::{
  fs, io,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  command::{CommandError, CommandRunner},
  diff::git::git_command,
  integration::{MergeError, RunMerge, WORKTREES_DIR, merge_run},
  project::TaskKey,
  revision::{Revision, RevisionError},
  run::Run,
  time::Timestamp,
};
use orchester_exec::ProcessRunner;

/// Looked up on the `PATH`, as the app and the runner look it up.
const GIT: &str = "git";
/// Every command here is local and small; one that takes longer than this hangs.
const RUNNER: ProcessRunner = ProcessRunner::new(Duration::from_secs(60));
/// The branch the repository starts on, which runs start from.
pub const MAIN: &str = "main";

#[derive(Debug, thiserror::Error)]
pub enum GitError {
  #[error("cannot run git: {0}")]
  Run(#[from] CommandError),
  #[error("`git {command}` failed: {stderr}")]
  Failed { command: String, stderr: String },
  #[error("git named {0:?} as a commit: {1}")]
  NotACommit(String, RevisionError),
  #[error("cannot write {path}: {source}")]
  Write {
    path: PathBuf,
    #[source]
    source: io::Error,
  },
}

/// `git <args>` in `dir`, which must succeed; its trimmed standard output.
fn git(dir: &Path, args: &[&str]) -> Result<String, GitError> {
  let output = RUNNER.run(&git_command(Path::new(GIT), dir, args.iter().copied()))?;
  if !output.success() {
    return Err(GitError::Failed {
      command: args.join(" "),
      stderr: output.stderr.trim().to_owned(),
    });
  }
  Ok(output.stdout.trim().to_owned())
}

/// The commit checked out in `dir`.
fn head(dir: &Path) -> Result<Revision, GitError> {
  let head = git(dir, &["rev-parse", "HEAD"])?;
  Revision::new(&head).map_err(|error| GitError::NotACommit(head, error))
}

/// Writes each `(path, content)` under `dir`, creating folders as needed.
pub fn write_files(dir: &Path, files: &[(&str, String)]) -> Result<(), GitError> {
  for (path, content) in files {
    let path = dir.join(path);
    if let Some(folder) = path.parent() {
      fs::create_dir_all(folder).map_err(|source| GitError::Write {
        path: folder.to_path_buf(),
        source,
      })?;
    }
    fs::write(&path, content).map_err(|source| GitError::Write { path, source })?;
  }
  Ok(())
}

/// Writes `files` in the checkout `dir` and commits everything with `message`; the commit.
pub fn commit(dir: &Path, files: &[(&str, String)], message: &str) -> Result<Revision, GitError> {
  write_files(dir, files)?;
  git(dir, &["add", "--all"])?;
  git(dir, &["commit", "--quiet", "-m", message])?;
  head(dir)
}

/// The repository at `root` has `url` as its `origin`.
pub fn add_origin(root: &Path, url: &str) -> Result<(), GitError> {
  git(root, &["remote", "add", "origin", url]).map(drop)
}

/// Makes `root` a repository on `main` with `files` in one commit, under a QA identity of its own so
/// the user's global identity and signing settings stay out of it; the commit.
pub fn init(root: &Path, files: &[(&str, String)]) -> Result<Revision, GitError> {
  git(root, &["init", "--quiet", &format!("--initial-branch={MAIN}")])?;
  for (key, value) in [
    ("user.name", "Kontur QA"),
    ("user.email", "qa@kontur.invalid"),
    ("core.autocrlf", "false"),
    ("commit.gpgsign", "false"),
  ] {
    git(root, &["config", key, value])?;
  }
  write_files(root, files)?;
  git(root, &["add", "--all"])?;
  git(
    root,
    &["commit", "--quiet", "-m", "Kontur: webhooks, exports and contacts"],
  )?;
  head(root)
}

/// A run's worktree, as the runner adds it: `<repo>/.orchester/worktrees/<run>` on the run's branch.
pub struct Worktree {
  repo: PathBuf,
  path: PathBuf,
}

impl Worktree {
  /// Adds the worktree named `name` on a new `branch` from `base`.
  pub fn add(repo: &Path, name: &str, branch: &str, base: &Revision) -> Result<Self, GitError> {
    let relative = format!("{WORKTREES_DIR}/{name}");
    git(
      repo,
      &["worktree", "add", "--quiet", "-b", branch, &relative, base.as_str()],
    )?;
    Ok(Self {
      repo: repo.to_path_buf(),
      path: repo.join(relative),
    })
  }

  /// Writes `files` and commits everything with `message`, as a checkpoint does; the commit.
  pub fn commit(&self, files: &[(&str, String)], message: &str) -> Result<Revision, GitError> {
    commit(&self.path, files, message)
  }

  /// Writes `files` and leaves them uncommitted: an agent's work since the last checkpoint.
  pub fn leave(&self, files: &[(&str, String)]) -> Result<(), GitError> {
    write_files(&self.path, files)
  }

  /// Removes the worktree folder; its branch and commits stay.
  pub fn remove(self) -> Result<(), GitError> {
    let path = self.path.to_string_lossy().into_owned();
    git(&self.repo, &["worktree", "remove", "--force", &path]).map(drop)
  }
}

/// Merges the branch of `run`, which succeeded, into the branch it started from in `repo`, as Merge into
/// main does; the merge to record. A temporary worktree that stayed fails the seed, which owns the repository.
pub fn merge(repo: &Path, (run, task): (&Run, &TaskKey), now: Timestamp) -> Result<RunMerge, MergeError> {
  let outcome = merge_run(&RUNNER, Path::new(GIT), repo, (run, task), now)?;
  match outcome.left {
    None => Ok(outcome.merge),
    Some(left) => Err(MergeError::Git(orchester_core::integration::GitError::Failed(
      left.detail,
    ))),
  }
}
