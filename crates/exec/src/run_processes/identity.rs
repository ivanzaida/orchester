use std::io;
#[cfg(target_os = "macos")]
use std::path::Path;

use orchester_core::integration::local::ProcessIdentity;

use super::{Entry, platform};

/// Observes only the PID just spawned by the local integration executor; it never enumerates or signals processes.
pub fn capture(pid: u32) -> io::Result<Option<ProcessIdentity>> {
  let process = platform::Process::open(Entry::pid(pid), false)?;
  Ok(process.map(|process| ProcessIdentity {
    pid,
    containment: None,
    executor: None,
    started: process.info.started,
    executable: crate::local_git::digest(process.info.path.to_string_lossy().as_bytes()),
  }))
}

/// A reused PID is not the recorded process. Unreadable identity is an error, never proof of completion.
pub fn still_running(identity: &ProcessIdentity) -> io::Result<bool> {
  Ok(capture(identity.pid)?.is_some_and(|current| current.pid == identity.pid && current.started == identity.started))
}

/// The identity of our own child `pid` that exited before [`capture`] could read it, while it is not reaped: its PID
/// stays ours until then and its start time stays readable, but its image no longer has a path, so the program it was
/// started from (`program`) stands for it. `None` once it is gone.
#[cfg(target_os = "macos")]
pub fn capture_exited_child(pid: u32, program: &Path) -> io::Result<Option<ProcessIdentity>> {
  Ok(platform::started(pid)?.map(|started| ProcessIdentity {
    pid,
    containment: None,
    executor: None,
    started,
    executable: crate::local_git::digest(program.to_string_lossy().as_bytes()),
  }))
}
