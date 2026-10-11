//! Retains the platform process identity while the runner calls the app's MCP server.

use std::{num::NonZeroU32, path::Path};

use thiserror::Error;

use super::{Entry, RunProcesses, ancestors, platform};

/// An inspected process whose executable belongs to this run, pinned across discovery and HTTP.
pub struct RunAppProcess {
  process: platform::Process,
}

#[derive(Debug, Error)]
pub enum RunAppProcessError {
  #[error("refused pid {0}: its live executable cannot be inspected")]
  Unreadable(u32),
  #[error("refused pid {pid}: {reason}")]
  Refused { pid: u32, reason: String },
  #[error("refused: the app exited or changed its process identity")]
  Changed,
}

impl RunProcesses {
  /// Applies exactly the stop tool's executable, age and protected-process rule without signalling anything.
  pub fn app(&self, pid: NonZeroU32) -> Result<RunAppProcess, RunAppProcessError> {
    let pid = pid.get();
    let process = platform::Process::open(Entry::pid(pid), false)
      .map_err(|_| RunAppProcessError::Unreadable(pid))?
      .ok_or(RunAppProcessError::Unreadable(pid))?;
    let entries = platform::snapshot().map_err(|_| RunAppProcessError::Unreadable(pid))?;
    self
      .validate(&process, &ancestors(&entries))
      .map_err(|reason| RunAppProcessError::Refused {
        pid,
        reason: reason.to_string(),
      })?;
    Ok(RunAppProcess { process })
  }
}

impl RunAppProcess {
  pub fn executable(&self) -> &Path {
    &self.process.info.path
  }

  /// Checks the retained Windows handle or macOS audit identity immediately before each request.
  pub fn check_alive(&self) -> Result<(), RunAppProcessError> {
    match self.process.alive() {
      Ok(true) => Ok(()),
      Ok(false) | Err(_) => Err(RunAppProcessError::Changed),
    }
  }
}
