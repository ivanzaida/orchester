//! Reading the `PATH` the user's login shell builds. The shell runs its start-up files, which may
//! print greetings or anything else on its output; the `PATH` is printed between two markers and
//! read from there, and what the shell prints on its error output is not read at all.
//!
//! Platform-neutral so it is tested on Windows too; only macOS reads a login shell
//! (`super::macos`).

use std::{
  io::{self, Read},
  path::{Path, PathBuf},
  process::{Child, ChildStdout, Command as Process, Stdio},
  sync::{
    Arc,
    mpsc::{self, Receiver, RecvTimeoutError},
  },
  thread,
  time::{Duration, Instant},
};

use thiserror::Error;

use super::SearchBasis;
use crate::console;

/// Printed right before the `PATH`.
pub const PATH_START: &str = "__ORCHESTER_PATH_START__";
/// Printed right after the `PATH`.
pub const PATH_END: &str = "__ORCHESTER_PATH_END__";
/// A login shell that has not printed its `PATH` by then is stopped, and its `PATH` is not used.
pub const TIMEOUT: Duration = Duration::from_secs(5);

const CHUNK_BYTES: usize = 4096;

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

/// What the reads so far found of the login shell's `PATH`: its folders, and the name of the shell that last gave them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoginShellPath {
  folders: Arc<[PathBuf]>,
  shell: Option<String>,
}

impl LoginShellPath {
  /// What is known after a read of `shell`: a read that failed keeps what an earlier one found, so *Check again* in a
  /// shell that stopped answering loses nothing.
  #[must_use]
  pub fn after_read(self, read: Result<Vec<PathBuf>, LoginShellError>, shell: &Path) -> Self {
    match read {
      Ok(folders) => Self {
        folders: folders.into(),
        shell: shell.file_name().map(|name| name.to_string_lossy().into_owned()),
      },
      Err(_) => self,
    }
  }

  pub const fn folders(&self) -> &Arc<[PathBuf]> {
    &self.folders
  }

  /// What a search with these folders can say it looked at: the login shell only once a read succeeded.
  pub fn basis(&self) -> SearchBasis {
    self
      .shell
      .clone()
      .map_or(SearchBasis::UsualFolders, |shell| SearchBasis::LoginShell { shell })
  }
}

/// The folders of the `PATH` that `shell` builds as an interactive login shell, so the start-up
/// files of both kinds run (`.zprofile` and `.zshrc`, `.bash_profile` and `.bashrc`).
pub fn read(shell: &Path, timeout: Duration) -> Result<Vec<PathBuf>, LoginShellError> {
  let script = format!("printf '\\n{PATH_START}%s{PATH_END}\\n' \"$PATH\"");
  read_with(shell, &["-l", "-i", "-c", &script], timeout)
}

/// Runs `program` with `args` and reads the `PATH` it prints between the markers on its standard
/// output, taking at most `timeout` in all. The program reads no input, and is stopped with
/// everything it started when it does not finish in time. A program started in the background by
/// the start-up files may keep the output open after the shell exited: the read ends as soon as
/// the `PATH` is there, and what still holds the output at the deadline is ended on macOS.
pub fn read_with(program: &Path, args: &[&str], timeout: Duration) -> Result<Vec<PathBuf>, LoginShellError> {
  let deadline = Instant::now() + timeout;
  let mut child = start(program, args)?;
  let chunks = read_chunks(child.stdout.take());
  // A shell whose start-up files fail may still print the PATH, so its exit status is not read.
  crate::wait_for(&mut child, program, timeout)
    .map_err(LoginShellError::Wait)?
    .ok_or(LoginShellError::TimedOut(timeout))?;
  match printed_path(&chunks, deadline)? {
    Some(folders) => Ok(folders),
    None => {
      // On Windows there is no group to end: the reader stops once what holds the output ends.
      #[cfg(unix)]
      end_left_behind(&child, program);
      Err(LoginShellError::TimedOut(timeout))
    }
  }
}

fn start(program: &Path, args: &[&str]) -> Result<Child, LoginShellError> {
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
  process.spawn().map_err(|source| LoginShellError::Start {
    shell: program.to_path_buf(),
    source,
  })
}

/// Sends what `pipe` gives as it comes, until its end, a read error, or nobody receives any more.
fn read_chunks(pipe: Option<ChildStdout>) -> Receiver<io::Result<Vec<u8>>> {
  let (sender, receiver) = mpsc::channel();
  thread::spawn(move || {
    let Some(mut pipe) = pipe else {
      return;
    };
    let mut buffer = [0; CHUNK_BYTES];
    loop {
      let chunk = match pipe.read(&mut buffer) {
        Ok(0) => return,
        Ok(read) => Ok(buffer[..read].to_vec()),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
        Err(error) => Err(error),
      };
      let failed = chunk.is_err();
      if sender.send(chunk).is_err() || failed {
        return;
      }
    }
  });
  receiver
}

/// The folders of the `PATH` in the output `chunks` bring, read until the output ends or holds the
/// `PATH`. `None` when `deadline` passes before either.
fn printed_path(
  chunks: &Receiver<io::Result<Vec<u8>>>,
  deadline: Instant,
) -> Result<Option<Vec<PathBuf>>, LoginShellError> {
  let mut output = Vec::new();
  loop {
    match chunks.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
      Ok(chunk) => output.extend(chunk.map_err(LoginShellError::Output)?),
      Err(RecvTimeoutError::Disconnected) => return parse(&String::from_utf8_lossy(&output)).map(Some),
      Err(RecvTimeoutError::Timeout) => return Ok(None),
    }
    // The shell printed the PATH last and has exited, so a whole PATH is all it will print.
    if let Ok(folders) = parse(&String::from_utf8_lossy(&output)) {
      return Ok(Some(folders));
    }
  }
}

/// Ends what the exited shell left running in its process group that still holds its output.
#[cfg(unix)]
fn end_left_behind(child: &Child, program: &Path) {
  let Some(group) = crate::process_group::ProcessGroup::led_by(child.id()) else {
    return;
  };
  if let Err(error) = group.kill() {
    tracing::warn!(%error, program = %program.display(), "cannot end what the login shell left running");
  }
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
