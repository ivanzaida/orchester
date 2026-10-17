//! `git` in one directory, for integration: the user's checkout or a run's worktree. Commands run
//! with git's messages in English, without optional locks ([`git_command`]) and without variables
//! that locate another repository ([`LOCATING`]); a failure is git's message without credentials.

use std::path::Path;

use super::GitError;
use crate::{
  command::{CommandOutput, CommandRunner},
  diff::git::git_command,
  repository::redact_userinfo,
  run::BranchName,
};

/// Variables that would point git at another repository, index, object store or ref namespace than
/// the folder it runs in, or give it configuration of the caller's, e.g. ones a hook that started
/// Orchester's process passed on. Without `GIT_CONFIG_COUNT`, git reads no `GIT_CONFIG_KEY_<n>` or
/// `GIT_CONFIG_VALUE_<n>`.
const LOCATING: [&str; 9] = [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_COMMON_DIR",
  "GIT_NAMESPACE",
  "GIT_CONFIG_PARAMETERS",
  "GIT_CONFIG_COUNT",
];

pub(super) struct Git<'a, R: CommandRunner> {
  runner: &'a R,
  program: &'a Path,
  dir: &'a Path,
}

impl<'a, R: CommandRunner> Git<'a, R> {
  pub(super) const fn new(runner: &'a R, program: &'a Path, dir: &'a Path) -> Self {
    Self { runner, program, dir }
  }

  /// The same git in `dir`.
  pub(super) const fn at(&self, dir: &'a Path) -> Self {
    Self::new(self.runner, self.program, dir)
  }

  /// The directory git runs in.
  pub(super) const fn dir(&self) -> &'a Path {
    self.dir
  }

  pub(super) fn output(&self, args: &[&str]) -> Result<CommandOutput, GitError> {
    let command = LOCATING.iter().fold(
      git_command(self.program, self.dir, args.iter().copied()),
      |command, name| command.without_env(name),
    );
    Ok(self.runner.run(&command)?)
  }

  /// Whether the command exits with 0, for queries whose every failure means "no".
  pub(super) fn succeeds(&self, args: &[&str]) -> Result<bool, GitError> {
    Ok(self.output(args)?.success())
  }

  /// Trimmed standard output of a command that must succeed.
  pub(super) fn stdout(&self, args: &[&str]) -> Result<String, GitError> {
    let output = self.output(args)?;
    if output.success() {
      Ok(output.stdout.trim().to_owned())
    } else {
      Err(GitError::Failed(message(&output)))
    }
  }

  /// The branch checked out here; `None` on a detached HEAD.
  pub(super) fn checked_out_branch(&self) -> Result<Option<BranchName>, GitError> {
    let output = self.output(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    if !output.success() {
      // `--quiet` fails silently on a detached HEAD only; anything else (no repository) says why.
      return if self.succeeds(&["rev-parse", "--verify", "--quiet", "HEAD"])? {
        Ok(None)
      } else {
        Err(GitError::Failed(message(&output)))
      };
    }
    let name = output.stdout.trim();
    BranchName::new(name).map(Some).map_err(|_| GitError::Unrecognized {
      expected: "a branch name",
      detail: name.to_owned(),
    })
  }

  /// Files with changes git has not committed here, untracked ones included, as `git status`
  /// names them. Ignored files are not listed.
  pub(super) fn uncommitted(&self) -> Result<Vec<String>, GitError> {
    // Not trimmed: the first entry's status can start with a space.
    let output = self.output(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])?;
    if !output.success() {
      return Err(GitError::Failed(message(&output)));
    }
    Ok(parse_status(&output.stdout))
  }

  /// Whether `ancestor` is reachable from `descendant` (both commit-ish).
  pub(super) fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool, GitError> {
    self.succeeds(&["merge-base", "--is-ancestor", ancestor, descendant])
  }

  /// The commit `name` points at; `None` when it names none.
  pub(super) fn commit(&self, name: &str) -> Result<Option<String>, GitError> {
    let target = format!("{name}^{{commit}}");
    let output = self.output(&["rev-parse", "--verify", "--quiet", "--end-of-options", &target])?;
    Ok(output.success().then(|| output.stdout.trim().to_owned()))
  }
}

/// What git said when a command failed: its error output, else its standard output (a merge
/// reports conflicts there).
pub(super) fn message(output: &CommandOutput) -> String {
  let text = if output.stderr.trim().is_empty() {
    &output.stdout
  } else {
    &output.stderr
  };
  redact_userinfo(text.trim())
}

/// The paths of `git status --porcelain=v1 -z`: `XY <path>` per entry, a rename or copy followed
/// by its original path, which is not listed again.
fn parse_status(status: &str) -> Vec<String> {
  let mut files = Vec::new();
  let mut entries = status.split('\0').filter(|entry| !entry.is_empty());
  while let Some(entry) = entries.next() {
    let (Some(code), Some(path)) = (entry.get(..2), entry.get(3..)) else {
      continue;
    };
    if code.contains(['R', 'C']) {
      entries.next();
    }
    files.push(path.to_owned());
  }
  files
}
