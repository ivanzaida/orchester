//! `git` in one directory for the runner: worktrees, the base revision of a run and the checkpoint
//! commits of its steps. Git runs through `orchester-exec` like every program Orchester starts (no
//! console window, a process group of its own on macOS, the full search path), with git's messages
//! in English and without optional locks ([`git_command`]), and with the user's own configuration,
//! so commits carry the repository's git identity.

use std::{path::Path, time::Duration};

use orchester_core::{
  command::{CommandOutput, CommandRunner},
  diff::git::git_command,
};
use orchester_exec::ProcessRunner;

use crate::worktree::WorktreeError;

/// Git work here changes the repository (adding the worktree of a large checkout, a commit or a
/// merge with the repository's hooks): stopping it halfway would leave the repository half
/// changed, so only a git that hangs is stopped.
const GIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const RUNNER: ProcessRunner = ProcessRunner::new(GIT_TIMEOUT);
const PROGRAM: &str = "git";

/// `git …` in `dir`.
pub(crate) struct Git<'a> {
  dir: &'a Path,
}

impl<'a> Git<'a> {
  pub(crate) fn new(dir: &'a Path) -> Self {
    Self { dir }
  }

  fn run(&self, args: &[&str]) -> Result<(String, CommandOutput), WorktreeError> {
    let output = RUNNER
      .run(&git_command(Path::new(PROGRAM), self.dir, args.iter().copied()))
      .map_err(WorktreeError::GitUnavailable)?;
    Ok((args.join(" "), output))
  }

  /// Whether the command exits with 0. For validations whose every failure means "no".
  pub(crate) fn succeeds(&self, args: &[&str]) -> Result<bool, WorktreeError> {
    Ok(self.run(args)?.1.success())
  }

  /// A yes/no query answered by exit code 0 or 1; any other exit is a git failure.
  pub(crate) fn answer(&self, args: &[&str]) -> Result<bool, WorktreeError> {
    let (command, output) = self.run(args)?;
    match output.code {
      Some(0) => Ok(true),
      Some(1) => Ok(false),
      _ => Err(failure(command, &output)),
    }
  }

  /// Trimmed standard output of a command that succeeded, or its trimmed standard error when it
  /// exited with a failure. For commands whose failure is an answer (a merge that conflicts).
  pub(crate) fn attempt(&self, args: &[&str]) -> Result<Result<String, String>, WorktreeError> {
    let (_, output) = self.run(args)?;
    Ok(if output.success() {
      Ok(output.stdout.trim().to_owned())
    } else {
      Err(output.stderr.trim().to_owned())
    })
  }

  /// Trimmed standard output of a command that must succeed.
  pub(crate) fn stdout(&self, args: &[&str]) -> Result<String, WorktreeError> {
    let (command, output) = self.run(args)?;
    if !output.success() {
      return Err(failure(command, &output));
    }
    Ok(output.stdout.trim().to_owned())
  }
}

fn failure(command: String, output: &CommandOutput) -> WorktreeError {
  WorktreeError::Git {
    command,
    code: output.code,
    stderr: output.stderr.trim().to_owned(),
  }
}
