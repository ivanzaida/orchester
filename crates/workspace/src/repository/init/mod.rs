//! Making a folder a git repository in place (project setup, `Create a git repository here` `OdlXb`): `git init` on
//! the branch git's `init.defaultBranch` names, else `main`, then a first commit "Initial commit" by the author git
//! is set up with, holding the folder's files (`git add -A`, which honours `.gitignore`) or nothing. Never adds a
//! remote, pushes, or changes git's settings, and git works on the folder only, whatever repository the caller's
//! environment names ([`git`]). Also what git sees of a folder before GitHub is asked and what a first commit would
//! hold ([`read`]). Every function blocks on git; a UI calls them off its thread.

mod git;
mod read;

use std::{io::ErrorKind, path::Path};

use orchester_core::command::{CommandError, CommandOutput, CommandRunner};
use orchester_store::is_repository_root;
use thiserror::Error;

use self::git::{command, has_commits, listed, said, staged};
pub use self::read::{
  Commits, InitialBranch, LocalRepository, Survey, SurveyError, initial_branch, read_local, survey,
};

/// The message of the first commit.
pub const INITIAL_COMMIT: &str = "Initial commit";
/// The branch a new repository starts on when git's `init.defaultBranch` names none.
pub const FALLBACK_BRANCH: &str = "main";

/// What the first commit holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstCommit {
  /// Every file `git add -A` adds: the folder's files except those `.gitignore` lists.
  Files,
  /// Nothing: the folder's files stay uncommitted.
  Empty,
}

/// A first commit made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Initialized {
  /// The files the commit holds; `None` when git did not say.
  pub files: Option<usize>,
}

/// The git command a failure happened in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitStep {
  Init,
  Add,
  Commit,
}

/// Why the repository or its first commit was not made. A failed step leaves what the steps before it made.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum InitError {
  #[error("git is not installed")]
  GitMissing,
  #[error("the folder cannot be written")]
  NotWritable,
  /// Refused: an existing repository is never touched.
  #[error("the folder is a git repository already")]
  AlreadyRepository,
  /// Refused: a first commit needs a repository.
  #[error("the folder is not a git repository")]
  NotRepository,
  /// Refused: the repository has its first commit already.
  #[error("the repository has a commit already")]
  HasCommits,
  /// Refused: an empty first commit would leave out what the person staged; the index is never emptied.
  #[error("the repository has staged files, which an empty first commit would leave out")]
  Staged,
  /// What git wrote when `git init` failed.
  #[error("git init failed")]
  InitFailed(String),
  /// What git wrote when `git add` failed; nothing is committed.
  #[error("git add failed")]
  AddFailed(String),
  /// What git wrote when `git commit` failed.
  #[error("git commit failed")]
  CommitFailed(String),
  /// `git commit` failed because git does not know who the author is; what git wrote.
  #[error("git does not know the commit's author")]
  NoIdentity(String),
  /// git could not be run to its end, e.g. it did not finish in time.
  #[error("git did not finish")]
  Stopped(InitStep),
}

/// Makes `folder` a git repository with its first commit holding `first`.
pub fn init_repository(
  runner: &dyn CommandRunner,
  folder: &Path,
  first: FirstCommit,
) -> Result<Initialized, InitError> {
  create_repository(runner, folder)?;
  make_first_commit(runner, folder, first)
}

/// Makes `folder` a git repository without a commit, on the branch [`initial_branch`] names; that branch.
/// A folder that is a repository already is refused and never touched.
pub fn create_repository(runner: &dyn CommandRunner, folder: &Path) -> Result<String, InitError> {
  if is_repository_root(folder) {
    return Err(InitError::AlreadyRepository);
  }
  writable(folder)?;
  let branch = initial_branch(runner)?;
  let initial = format!("--initial-branch={}", branch.name());
  run(runner, folder, &["init", "--quiet", initial.as_str()], InitStep::Init).map_err(|failed| match failed {
    Failed::Said(text) => InitError::InitFailed(text),
    Failed::Halted(error) => error,
  })?;
  Ok(branch.name().to_owned())
}

/// Makes the first commit of the repository in `folder`, holding `first`, by the author git is set up with. A
/// folder that is no repository, a repository that has a commit, and an empty first commit while something is staged
/// are refused.
pub fn make_first_commit(
  runner: &dyn CommandRunner,
  folder: &Path,
  first: FirstCommit,
) -> Result<Initialized, InitError> {
  if !is_repository_root(folder) {
    return Err(InitError::NotRepository);
  }
  match has_commits(runner, folder) {
    Ok(false) => {}
    Ok(true) => return Err(InitError::HasCommits),
    Err(error) => return Err(halted(&error, InitStep::Commit)),
  }
  let mut commit = vec!["commit", "--quiet", "-m", INITIAL_COMMIT];
  match first {
    FirstCommit::Files => {
      run(runner, folder, &["add", "-A"], InitStep::Add).map_err(|failed| match failed {
        Failed::Said(text) => InitError::AddFailed(text),
        Failed::Halted(error) => error,
      })?;
    }
    FirstCommit::Empty => {
      match staged(runner, folder) {
        Ok(0) => {}
        Ok(_) => return Err(InitError::Staged),
        Err(error) => return Err(halted(&error, InitStep::Commit)),
      }
      commit.push("--allow-empty");
    }
  }
  if let Err(failed) = run(runner, folder, &commit, InitStep::Commit) {
    return Err(match failed {
      Failed::Said(text) if !identity_known(runner, folder) => InitError::NoIdentity(text),
      Failed::Said(text) => InitError::CommitFailed(text),
      Failed::Halted(error) => error,
    });
  }
  Ok(Initialized {
    files: committed_files(runner, folder),
  })
}

/// A file can be made in `folder`; the one made to find out is removed at once.
fn writable(folder: &Path) -> Result<(), InitError> {
  match tempfile::Builder::new().prefix(".orchester-").tempfile_in(folder) {
    Ok(_) => Ok(()),
    Err(error)
      if matches!(
        error.kind(),
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem
      ) =>
    {
      Err(InitError::NotWritable)
    }
    // Anything else is for git to say.
    Err(_) => Ok(()),
  }
}

/// How a git command failed.
enum Failed {
  /// It ran and failed; what it wrote.
  Said(String),
  Halted(InitError),
}

fn run(runner: &dyn CommandRunner, folder: &Path, args: &[&str], step: InitStep) -> Result<CommandOutput, Failed> {
  let output = runner
    .run(&command(folder, args.iter().copied()))
    .map_err(|error| Failed::Halted(halted(&error, step)))?;
  if output.success() {
    Ok(output)
  } else {
    Err(Failed::Said(said(&output)))
  }
}

fn halted(error: &CommandError, step: InitStep) -> InitError {
  match error {
    CommandError::NotFound => InitError::GitMissing,
    _ => InitError::Stopped(step),
  }
}

/// The files of the commit at `HEAD`; `None` when git does not say.
fn committed_files(runner: &dyn CommandRunner, folder: &Path) -> Option<usize> {
  runner
    .run(&command(folder, ["ls-tree", "-r", "-z", "--name-only", "HEAD"]))
    .ok()
    .filter(CommandOutput::success)
    .map(|output| listed(&output))
}

/// git can name both the author and the committer of a commit, from its settings or the environment.
fn identity_known(runner: &dyn CommandRunner, folder: &Path) -> bool {
  ["GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"].into_iter().all(|ident| {
    runner
      .run(&command(folder, ["var", ident]))
      .is_ok_and(|output| output.success())
  })
}
