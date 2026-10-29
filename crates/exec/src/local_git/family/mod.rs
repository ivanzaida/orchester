//! Local integration never treats parent exit as proof that its writers have stopped: each program it runs is held
//! with everything it starts, in a private job on Windows and in a process group of its own on macOS.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
use orchester_core::{blob::ContentHash, integration::local::ProcessIdentity};
#[cfg(unix)]
pub(super) use unix::Family;
#[cfg(windows)]
pub(super) use windows::Family;

use super::{LocalError, LocalGitError};

/// Reconciliation only observes the persisted job or group, and never signals a PID or adopts foreign processes.
pub fn stopped(identity: &ProcessIdentity) -> Result<bool, LocalGitError> {
  #[cfg(windows)]
  {
    windows::stopped(identity)
  }
  #[cfg(unix)]
  {
    unix::stopped(identity)
  }
}

/// An unfinished receipt cannot be completed while its original executor may still hold blocked pipe workers.
pub fn reconcile_stopped(identity: &ProcessIdentity) -> Result<bool, LocalGitError> {
  let executor = identity.executor.as_ref().ok_or(LocalError::ProcessUncertain)?;
  if crate::run_processes::identity::capture(executor.pid)
    .map_err(|_| LocalError::ProcessUncertain)?
    .is_some_and(|current| current.started == executor.started)
  {
    return Err(LocalError::ProcessUncertain.into());
  }
  stopped(identity)
}

/// `identity` of a program started in the family whose private `token` it records, with this executor's.
fn record(token: &ContentHash, mut identity: ProcessIdentity) -> Result<ProcessIdentity, LocalGitError> {
  identity.containment = Some(token.clone());
  let executor = crate::run_processes::identity::capture(std::process::id())
    .map_err(|_| LocalError::ProcessUncertain)?
    .ok_or(LocalError::ProcessUncertain)?;
  identity.executor = Some(orchester_core::integration::local::ExecutorIdentity {
    pid: executor.pid,
    started: executor.started,
  });
  Ok(identity)
}
