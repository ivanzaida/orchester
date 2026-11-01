//! The clone in a Terminal window, as the agent CLIs' sign-in opens one (`sign_in_terminal`): Terminal runs a
//! `.command` script of this module's own ([`super::terminal_script`]). Whether the window still runs is told by the
//! process id the script writes first: once that process is gone without a result, the window was closed.

use std::{
  fs, io,
  os::unix::fs::PermissionsExt,
  path::{Path, PathBuf},
  process::Command,
  thread,
  time::{Duration, Instant},
};

use rustix::process::{Pid, test_kill_process};

use super::{CloneJob, ConsoleEnd, ConsoleError, POLL, Scratch, follow, terminal_script};

/// How long Terminal may take to start the script before the clone counts as not started.
const START: Duration = Duration::from_secs(120);

#[derive(Debug)]
pub(super) struct Running {
  pid_file: PathBuf,
  result: PathBuf,
}

pub(super) fn open(job: &CloneJob<'_>, files: &Scratch) -> Result<Running, ConsoleError> {
  let (script, pid_file, result) = (files.file("clone.command"), files.file("pid"), files.result());
  let text = terminal_script(job, &pid_file, &result)?;
  fs::write(&script, text).map_err(ConsoleError::NotOpened)?;
  fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).map_err(ConsoleError::NotOpened)?;
  let status = Command::new("open")
    .args(["-a", "Terminal"])
    .arg(&script)
    .status()
    .map_err(ConsoleError::NotOpened)?;
  if !status.success() {
    return Err(ConsoleError::NotOpened(io::Error::other(format!(
      "Terminal did not open ({status})"
    ))));
  }
  Ok(Running { pid_file, result })
}

pub(super) fn wait(running: Running, _files: &Scratch) -> Result<ConsoleEnd, ConsoleError> {
  let pid = started(&running.pid_file)?;
  follow(&running.result, || Ok(alive(pid)))
}

/// The script's process id, once it wrote it; [`ConsoleError::NotStarted`] when Terminal never ran it.
fn started(pid_file: &Path) -> Result<Pid, ConsoleError> {
  let deadline = Instant::now() + START;
  loop {
    match fs::read_to_string(pid_file) {
      Ok(text) => {
        return text
          .trim()
          .parse::<i32>()
          .ok()
          .and_then(Pid::from_raw)
          .ok_or(ConsoleError::UnreadableResult);
      }
      Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(ConsoleError::Wait(error)),
      Err(_) if Instant::now() >= deadline => return Err(ConsoleError::NotStarted),
      Err(_) => thread::sleep(POLL),
    }
  }
}

/// Whether the process `pid` still runs: a signal test it is gone from answers no.
fn alive(pid: Pid) -> bool {
  match test_kill_process(pid) {
    Ok(()) => true,
    Err(error) => error != rustix::io::Errno::SRCH,
  }
}
