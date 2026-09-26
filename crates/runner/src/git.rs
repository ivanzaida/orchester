//! `git -C <dir> …` for the runner: worktrees, the base revision of a run and the checkpoint
//! commits of its steps. Runs with the user's own git and configuration, so commits carry the
//! repository's git identity.

use std::{
  ffi::OsStr,
  path::Path,
  process::{Command, Output},
};

use crate::worktree::WorktreeError;

/// `git -C <dir> …`.
pub(crate) struct Git<'a> {
  dir: &'a Path,
}

impl<'a> Git<'a> {
  pub(crate) fn new(dir: &'a Path) -> Self {
    Self { dir }
  }

  fn run<I, S>(&self, args: I) -> Result<(String, Output), WorktreeError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let args: Vec<_> = args.into_iter().map(|arg| arg.as_ref().to_os_string()).collect();
    let mut git = Command::new("git");
    git.arg("-C").arg(self.dir).args(&args);
    orchester_exec::hide_window(&mut git);
    let output = git.output().map_err(WorktreeError::GitUnavailable)?;
    let command = args
      .iter()
      .map(|arg| arg.to_string_lossy())
      .collect::<Vec<_>>()
      .join(" ");
    Ok((command, output))
  }

  /// Whether the command exits with 0. For validations whose every failure means "no".
  pub(crate) fn succeeds<I, S>(&self, args: I) -> Result<bool, WorktreeError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    Ok(self.run(args)?.1.status.success())
  }

  /// A yes/no query answered by exit code 0 or 1; any other exit is a git failure.
  pub(crate) fn answer<I, S>(&self, args: I) -> Result<bool, WorktreeError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let (command, output) = self.run(args)?;
    match output.status.code() {
      Some(0) => Ok(true),
      Some(1) => Ok(false),
      _ => Err(failure(command, &output)),
    }
  }

  /// Trimmed standard output of a command that succeeded, or its trimmed standard error when it
  /// exited with a failure. For commands whose failure is an answer (a merge that conflicts).
  pub(crate) fn attempt<I, S>(&self, args: I) -> Result<Result<String, String>, WorktreeError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let (_, output) = self.run(args)?;
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).trim().to_owned();
    Ok(if output.status.success() {
      Ok(text(&output.stdout))
    } else {
      Err(text(&output.stderr))
    })
  }

  /// Trimmed standard output of a command that must succeed.
  pub(crate) fn stdout<I, S>(&self, args: I) -> Result<String, WorktreeError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let (command, output) = self.run(args)?;
    if !output.status.success() {
      return Err(failure(command, &output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
  }
}

fn failure(command: String, output: &Output) -> WorktreeError {
  WorktreeError::Git {
    command,
    code: output.status.code(),
    stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
  }
}
