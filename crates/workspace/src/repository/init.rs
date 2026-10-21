//! Making a folder a git repository in place (project setup, `Create a git repository here` `OdlXb`): `git init` on
//! the branch git's `init.defaultBranch` names, else `main`, then a first commit "Initial commit" by the author git
//! is set up with, holding the folder's files (`git add -A`, which honours `.gitignore`) or nothing. Never adds a
//! remote, pushes, or changes git's settings. Also what git sees of a folder before GitHub is asked ([`read_local`])
//! and what a first commit would hold ([`survey`]). Every function blocks on git; a UI calls them off its thread.

use std::{io::ErrorKind, path::Path};

use orchester_core::command::{Command, CommandError, CommandOutput, CommandRunner};
use orchester_store::is_repository_root;
use thiserror::Error;

/// The message of the first commit.
pub const INITIAL_COMMIT: &str = "Initial commit";
/// The branch a new repository starts on when git's `init.defaultBranch` names none.
pub const FALLBACK_BRANCH: &str = "main";
const GIT: &str = "git";

/// What the first commit holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstCommit {
  /// Every file `git add -A` adds: the folder's files except those `.gitignore` lists.
  Files,
  /// Nothing: the folder's files stay uncommitted.
  Empty,
}

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
  pub branch: InitialBranch,
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
  /// `count` is `None` when git could not count them.
  Commits {
    branch: Option<String>,
    count: Option<u64>,
  },
  /// git answered in a way that says nothing of the repository, e.g. it refuses a folder another user owns.
  Unread,
}

/// A repository made with its first commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Initialized {
  pub branch: String,
  /// The files the first commit holds.
  pub files: usize,
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

/// Why a folder could not be surveyed.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SurveyError {
  #[error("git is not installed")]
  GitMissing,
  #[error("git could not list the folder's files")]
  Unreadable,
}

/// What git sees of `folder`.
pub fn read_local(runner: &dyn CommandRunner, folder: &Path) -> LocalRepository {
  if !is_repository_root(folder) {
    // Not in the folder: a folder that is gone would read as a missing program.
    return match runner.run(&Command::new(Path::new(GIT), ["--version"])) {
      Err(CommandError::NotFound) => LocalRepository::GitMissing,
      _ => LocalRepository::NotRepository,
    };
  }
  let branch = || current_branch(runner, folder);
  match has_commits(runner, folder) {
    Ok(true) => LocalRepository::Commits {
      branch: branch(),
      count: commit_count(runner, folder),
    },
    Ok(false) => LocalRepository::NoCommits { branch: branch() },
    Err(CommandError::NotFound) => LocalRepository::GitMissing,
    Err(_) => LocalRepository::Unread,
  }
}

/// What a first commit of `folder` would hold: the files `git add -A` adds there, counted without making
/// anything in the folder, and the branch `git init` would make.
pub fn survey(runner: &dyn CommandRunner, folder: &Path) -> Result<Survey, SurveyError> {
  let branch = initial_branch(runner).map_err(|error| match error {
    InitError::GitMissing => SurveyError::GitMissing,
    _ => SurveyError::Unreadable,
  })?;
  let files = if is_repository_root(folder) {
    untracked_files(runner, folder, &[])?
  } else {
    // A repository of its own outside the folder, so nothing is made there and no parent repository is asked.
    let scratch = tempfile::Builder::new()
      .prefix("orchester-survey-")
      .tempdir()
      .map_err(|_| SurveyError::Unreadable)?;
    let git_dir = scratch.path().to_string_lossy().into_owned();
    let init = runner.run(&Command::new(
      Path::new(GIT),
      ["init", "--quiet", "--bare", git_dir.as_str()],
    ));
    match init {
      Ok(output) if output.success() => {}
      Err(CommandError::NotFound) => return Err(SurveyError::GitMissing),
      _ => return Err(SurveyError::Unreadable),
    }
    let work_tree = folder.to_string_lossy().into_owned();
    untracked_files(
      runner,
      folder,
      &["--git-dir", git_dir.as_str(), "--work-tree", work_tree.as_str()],
    )?
  };
  Ok(Survey {
    files,
    gitignore: folder.join(".gitignore").is_file(),
    branch,
  })
}

/// The files git would add in `folder`: untracked ones `.gitignore` does not list, and those staged already.
fn untracked_files(runner: &dyn CommandRunner, folder: &Path, repository: &[&str]) -> Result<usize, SurveyError> {
  let args = repository
    .iter()
    .copied()
    .chain(["ls-files", "-z", "--cached", "--others", "--exclude-standard"]);
  match runner.run(&Command::new(Path::new(GIT), args).with_current_dir(folder)) {
    Ok(output) if output.success() => Ok(output.stdout.split('\0').filter(|name| !name.is_empty()).count()),
    Err(CommandError::NotFound) => Err(SurveyError::GitMissing),
    _ => Err(SurveyError::Unreadable),
  }
}

/// The branch `git init` makes: the one git's `init.defaultBranch` names, else [`FALLBACK_BRANCH`].
pub fn initial_branch(runner: &dyn CommandRunner) -> Result<InitialBranch, InitError> {
  // Outside any repository, as `git init` reads its settings: the user's and the system's, never a parent's.
  let command =
    Command::new(Path::new(GIT), ["config", "--get", "init.defaultBranch"]).with_current_dir(&std::env::temp_dir());
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
  git(runner, folder, &["init", "--quiet", initial.as_str()], InitStep::Init).map_err(|failed| match failed {
    Failed::Said(text) => InitError::InitFailed(text),
    Failed::Halted(error) => error,
  })?;
  Ok(branch.name)
}

/// Makes the first commit of the repository in `folder`, holding `first`, by the author git is set up with. A
/// folder that is no repository, and a repository that has a commit, are refused.
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
  // An empty commit holds nothing, even what an earlier `git add` left staged.
  let stage: &[&str] = match first {
    FirstCommit::Files => &["add", "-A"],
    FirstCommit::Empty => &["read-tree", "--empty"],
  };
  git(runner, folder, stage, InitStep::Add).map_err(|failed| match failed {
    Failed::Said(text) => InitError::AddFailed(text),
    Failed::Halted(error) => error,
  })?;
  let mut commit = vec!["commit", "--quiet", "-m", INITIAL_COMMIT];
  if first == FirstCommit::Empty {
    commit.push("--allow-empty");
  }
  if let Err(failed) = git(runner, folder, &commit, InitStep::Commit) {
    return Err(match failed {
      Failed::Said(text) if !identity_known(runner, folder) => InitError::NoIdentity(text),
      Failed::Said(text) => InitError::CommitFailed(text),
      Failed::Halted(error) => error,
    });
  }
  Ok(Initialized {
    branch: current_branch(runner, folder).unwrap_or_default(),
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

fn git(runner: &dyn CommandRunner, folder: &Path, args: &[&str], step: InitStep) -> Result<CommandOutput, Failed> {
  let output = runner
    .run(&Command::new(Path::new(GIT), args.iter().copied()).with_current_dir(folder))
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

/// What git wrote about a failure: its error output, or its output when it wrote its reason there.
fn said(output: &CommandOutput) -> String {
  let error = output.stderr.trim();
  if error.is_empty() {
    output.stdout.trim().to_owned()
  } else {
    error.to_owned()
  }
}

fn has_commits(runner: &dyn CommandRunner, folder: &Path) -> Result<bool, CommandError> {
  let output = runner.run(
    &Command::new(Path::new(GIT), ["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]).with_current_dir(folder),
  )?;
  match output.code {
    Some(0) => Ok(true),
    Some(1) => Ok(false),
    _ => Err(CommandError::Failed(said(&output))),
  }
}

/// The branch `HEAD` is on; `None` on a detached `HEAD` or when git does not say.
fn current_branch(runner: &dyn CommandRunner, folder: &Path) -> Option<String> {
  let output = runner
    .run(&Command::new(Path::new(GIT), ["symbolic-ref", "--quiet", "--short", "HEAD"]).with_current_dir(folder))
    .ok()?;
  let branch = output.stdout.trim();
  (output.success() && !branch.is_empty()).then(|| branch.to_owned())
}

fn commit_count(runner: &dyn CommandRunner, folder: &Path) -> Option<u64> {
  let output = runner
    .run(&Command::new(Path::new(GIT), ["rev-list", "--count", "HEAD"]).with_current_dir(folder))
    .ok()?;
  output.success().then(|| output.stdout.trim().parse().ok()).flatten()
}

/// The files of the commit at `HEAD`; 0 when git does not say.
fn committed_files(runner: &dyn CommandRunner, folder: &Path) -> usize {
  runner
    .run(&Command::new(Path::new(GIT), ["ls-tree", "-r", "-z", "--name-only", "HEAD"]).with_current_dir(folder))
    .ok()
    .filter(CommandOutput::success)
    .map_or(0, |output| {
      output.stdout.split('\0').filter(|name| !name.is_empty()).count()
    })
}

/// git can name both the author and the committer of a commit, from its settings or the environment.
fn identity_known(runner: &dyn CommandRunner, folder: &Path) -> bool {
  ["GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"].into_iter().all(|ident| {
    runner
      .run(&Command::new(Path::new(GIT), ["var", ident]).with_current_dir(folder))
      .is_ok_and(|output| output.success())
  })
}
