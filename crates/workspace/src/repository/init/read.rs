//! What git sees of a folder before anything is made there: whether it is a repository and has commits
//! ([`read_local`]), and what a first commit would hold and on which branch ([`survey`]).

use std::{
  env,
  path::{Path, PathBuf},
};

use orchester_core::command::{CommandError, CommandRunner};
use orchester_store::is_repository_root;
use thiserror::Error;

use super::{
  FALLBACK_BRANCH, InitError, InitStep,
  git::{command, current_branch, has_commits, listed, said, staged},
  halted,
};

/// The branch `git init` makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitialBranch {
  name: String,
  /// Named by git's `init.defaultBranch`, rather than [`FALLBACK_BRANCH`].
  configured: bool,
}

impl InitialBranch {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub const fn is_configured(&self) -> bool {
    self.configured
  }
}

/// What a first commit of a folder would hold, and the branch `git init` would make there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Survey {
  /// The files `git add -A` would add.
  pub files: usize,
  /// The folder has a `.gitignore`, which keeps what it lists out.
  pub gitignore: bool,
  /// The paths staged already in the folder's repository, which an empty first commit would leave out.
  pub staged: usize,
  pub branch: InitialBranch,
  /// The top of the repository the folder is inside of, when it is not a repository's top itself: a repository
  /// made here would be one inside that one.
  pub enclosing: Option<PathBuf>,
}

/// Why a folder could not be surveyed.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SurveyError {
  #[error("git is not installed")]
  GitMissing,
  #[error("git could not list the folder's files")]
  Unreadable,
}

/// How many commits a repository has, counted up to two: a long history is not walked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Commits {
  One,
  Several,
  /// git did not say.
  Unknown,
}

/// The folder as git sees it, before GitHub is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalRepository {
  /// git is not installed on this computer.
  GitMissing,
  NotRepository,
  /// A repository without a commit, so runs have nothing to branch from. `branch` is `None` on a detached `HEAD`.
  NoCommits {
    branch: Option<String>,
  },
  Commits {
    branch: Option<String>,
    commits: Commits,
  },
  /// git answered in a way that says nothing of the repository, e.g. it refuses a folder another user owns.
  Unread,
}

/// What git sees of `folder`.
pub fn read_local(runner: &dyn CommandRunner, folder: &Path) -> LocalRepository {
  if !is_repository_root(folder) {
    // Not in the folder: a folder that is gone would read as a missing program.
    return match runner.run(&command(&env::temp_dir(), ["--version"])) {
      Err(CommandError::NotFound) => LocalRepository::GitMissing,
      _ => LocalRepository::NotRepository,
    };
  }
  let branch = || current_branch(runner, folder);
  match has_commits(runner, folder) {
    Ok(true) => LocalRepository::Commits {
      branch: branch(),
      commits: commits(runner, folder),
    },
    Ok(false) => LocalRepository::NoCommits { branch: branch() },
    Err(CommandError::NotFound) => LocalRepository::GitMissing,
    Err(_) => LocalRepository::Unread,
  }
}

fn commits(runner: &dyn CommandRunner, folder: &Path) -> Commits {
  let count = runner
    .run(&command(folder, ["rev-list", "--count", "--max-count=2", "HEAD"]))
    .ok()
    .filter(|output| output.success())
    .and_then(|output| output.stdout.trim().parse::<u64>().ok());
  match count {
    Some(1) => Commits::One,
    Some(2..) => Commits::Several,
    Some(0) | None => Commits::Unknown,
  }
}

/// What a first commit of `folder` would hold: the files `git add -A` adds there, counted without making
/// anything in the folder, what is staged already, the branch `git init` would make, and the repository the folder
/// is inside of.
pub fn survey(runner: &dyn CommandRunner, folder: &Path) -> Result<Survey, SurveyError> {
  let branch = initial_branch(runner).map_err(|error| match error {
    InitError::GitMissing => SurveyError::GitMissing,
    _ => SurveyError::Unreadable,
  })?;
  let unreadable = |error: CommandError| match error {
    CommandError::NotFound => SurveyError::GitMissing,
    _ => SurveyError::Unreadable,
  };
  let (files, staged, enclosing) = if is_repository_root(folder) {
    let files = untracked_files(runner, folder, &[])?;
    (files, staged(runner, folder).map_err(unreadable)?, None)
  } else {
    (scratch_files(runner, folder)?, 0, enclosing(runner, folder))
  };
  Ok(Survey {
    files,
    gitignore: folder.join(".gitignore").is_file(),
    staged,
    branch,
    enclosing,
  })
}

/// The files `git add -A` would add in `folder`, which is no repository, through a repository of its own outside
/// the folder, so nothing is made there and no repository the folder is inside of is asked.
fn scratch_files(runner: &dyn CommandRunner, folder: &Path) -> Result<usize, SurveyError> {
  let scratch = tempfile::Builder::new()
    .prefix("orchester-survey-")
    .tempdir()
    .map_err(|_| SurveyError::Unreadable)?;
  let git_dir = scratch.path().to_string_lossy().into_owned();
  match runner.run(&command(
    &env::temp_dir(),
    ["init", "--quiet", "--bare", git_dir.as_str()],
  )) {
    Ok(output) if output.success() => {}
    Err(CommandError::NotFound) => return Err(SurveyError::GitMissing),
    _ => return Err(SurveyError::Unreadable),
  }
  let work_tree = folder.to_string_lossy().into_owned();
  untracked_files(
    runner,
    folder,
    &["--git-dir", git_dir.as_str(), "--work-tree", work_tree.as_str()],
  )
}

/// The files git would add in `folder`: untracked ones `.gitignore` does not list, and those staged already.
fn untracked_files(runner: &dyn CommandRunner, folder: &Path, repository: &[&str]) -> Result<usize, SurveyError> {
  let args = repository
    .iter()
    .copied()
    .chain(["ls-files", "-z", "--cached", "--others", "--exclude-standard"]);
  match runner.run(&command(folder, args)) {
    Ok(output) if output.success() => Ok(listed(&output)),
    Err(CommandError::NotFound) => Err(SurveyError::GitMissing),
    _ => Err(SurveyError::Unreadable),
  }
}

/// The top of the repository `folder` is inside of; `None` when it is inside none, or git does not say.
fn enclosing(runner: &dyn CommandRunner, folder: &Path) -> Option<PathBuf> {
  let output = runner
    .run(&command(folder, ["rev-parse", "--show-toplevel"]))
    .ok()
    .filter(|output| output.success())?;
  let top = output.stdout.trim();
  (!top.is_empty()).then(|| PathBuf::from(top))
}

/// The branch `git init` makes: the one git's `init.defaultBranch` names, else [`FALLBACK_BRANCH`].
pub fn initial_branch(runner: &dyn CommandRunner) -> Result<InitialBranch, InitError> {
  // Outside any repository, as `git init` reads its settings: the user's and the system's, never a parent's.
  let command = command(&env::temp_dir(), ["config", "--get", "init.defaultBranch"]);
  let output = runner.run(&command).map_err(|error| halted(&error, InitStep::Init))?;
  let named = output.stdout.trim();
  Ok(match output.code {
    Some(0) if !named.is_empty() => InitialBranch {
      name: named.to_owned(),
      configured: true,
    },
    // 1: the setting is not set.
    Some(0 | 1) => InitialBranch {
      name: FALLBACK_BRANCH.to_owned(),
      configured: false,
    },
    _ => return Err(InitError::InitFailed(said(&output))),
  })
}
