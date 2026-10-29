//! Reading the `PATH` the user's login shell builds. The shell runs its start-up files, which may
//! print greetings or anything else on its output; the `PATH` is printed between two markers and
//! read from there, and what the shell prints on its error output is not read at all.
//!
//! Platform-neutral so it is tested on Windows too; only macOS reads a login shell
//! (`super::macos`).

use std::{
  io,
  path::{Path, PathBuf},
  process::{Command as Process, Stdio},
  time::Duration,
};

use thiserror::Error;

use crate::console;

/// Printed right before the `PATH`.
pub const PATH_START: &str = "__ORCHESTER_PATH_START__";
/// Printed right after the `PATH`.
pub const PATH_END: &str = "__ORCHESTER_PATH_END__";
/// A login shell that has not printed its `PATH` by then is stopped, and its `PATH` is not used.
pub const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum LoginShellError {
  #[error("cannot start the shell {}: {source}", shell.display())]
  Start { shell: PathBuf, source: io::Error },
  #[error("the shell did not print its PATH within {0:?}")]
  TimedOut(Duration),
  #[error("cannot wait for the shell: {0}")]
  Wait(io::Error),
  #[error("cannot read the shell's output: {0}")]
  Output(io::Error),
  #[error("the shell's output has no PATH between the markers")]
  NoPath,
}

/// The folders of the `PATH` that `shell` builds as an interactive login shell, so the start-up
/// files of both kinds run (`.zprofile` and `.zshrc`, `.bash_profile` and `.bashrc`).
pub fn read(shell: &Path, timeout: Duration) -> Result<Vec<PathBuf>, LoginShellError> {
  let script = format!("printf '\\n{PATH_START}%s{PATH_END}\\n' \"$PATH\"");
  read_with(shell, &["-l", "-i", "-c", &script], timeout)
}

/// Runs `program` with `args` and reads the `PATH` it prints between the markers on its standard
/// output. The program reads no input, and is stopped with everything it started when it does not
/// finish within `timeout`.
pub fn read_with(program: &Path, args: &[&str], timeout: Duration) -> Result<Vec<PathBuf>, LoginShellError> {
  // Not `crate::prepare`: that gives the program the search path, which this read is part of.
  let mut process = Process::new(program);
  process
    .args(args)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
  console::hide_window(&mut process);
  #[cfg(unix)]
  crate::process_group::lead_own_group(&mut process);
  let mut child = process.spawn().map_err(|source| LoginShellError::Start {
    shell: program.to_path_buf(),
    source,
  })?;
  let stdout = crate::read_to_end(child.stdout.take());
  // A shell whose start-up files fail may still print the PATH, so its exit status is not read.
  crate::wait_for(&mut child, program, timeout)
    .map_err(LoginShellError::Wait)?
    .ok_or(LoginShellError::TimedOut(timeout))?;
  let output = stdout
    .join()
    .map_err(|_| LoginShellError::Output(io::Error::other("the output reader panicked")))?
    .map_err(LoginShellError::Output)?;
  parse(&output)
}

/// The folders of the `PATH` in `stdout`: between the last [`PATH_START`] and the [`PATH_END`]
/// after it, split on `:` (a macOS `PATH`, whichever system reads it). Empty and relative entries
/// are left out.
pub fn parse(stdout: &str) -> Result<Vec<PathBuf>, LoginShellError> {
  let (_, after) = stdout.rsplit_once(PATH_START).ok_or(LoginShellError::NoPath)?;
  let (path, _) = after.split_once(PATH_END).ok_or(LoginShellError::NoPath)?;
  Ok(
    path
      .split(':')
      .filter(|entry| entry.starts_with('/'))
      .map(PathBuf::from)
      .collect(),
  )
}
