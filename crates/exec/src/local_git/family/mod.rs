//! Local integration never treats parent exit as proof that its writers have stopped.
#[cfg(windows)]
mod windows;
use orchester_core::integration::local::ProcessIdentity;
#[cfg(windows)]
pub(super) use windows::Family;

#[cfg(not(windows))]
use super::LocalError;
use super::LocalGitError;

/// Reconciliation only observes the persisted job, and never signals a PID or adopts foreign processes.
pub fn stopped(identity: &ProcessIdentity) -> Result<bool, LocalGitError> {
  #[cfg(windows)]
  {
    windows::stopped(identity)
  }
  #[cfg(not(windows))]
  {
    let _identity = identity;
    Err(LocalError::ProcessContainmentUnsupported.into())
  }
}

#[cfg(not(windows))]
pub(super) struct Family;
#[cfg(not(windows))]
impl Family {
  pub(super) fn create() -> Result<Self, LocalGitError> {
    Err(LocalError::ProcessContainmentUnsupported.into())
  }
  pub(super) fn spawn(&self, _command: &mut std::process::Command) -> Result<std::process::Child, LocalGitError> {
    Err(LocalError::ProcessContainmentUnsupported.into())
  }
  pub(super) fn identity(&self, _child: &std::process::Child) -> Result<ProcessIdentity, LocalGitError> {
    Err(LocalError::ProcessContainmentUnsupported.into())
  }
  pub(super) fn resume(&self, _child: &std::process::Child) -> Result<(), LocalGitError> {
    Err(LocalError::ProcessContainmentUnsupported.into())
  }
  pub(super) fn empty(&self) -> Result<bool, LocalGitError> {
    Err(LocalError::ProcessContainmentUnsupported.into())
  }
  pub(super) fn stop(&self) -> Result<(), LocalGitError> {
    Err(LocalError::ProcessContainmentUnsupported.into())
  }
}
/// An unfinished receipt cannot be completed while its original executor may still hold blocked pipe workers.
pub fn reconcile_stopped(identity: &ProcessIdentity) -> Result<bool, LocalGitError> {
  let executor = identity.executor.as_ref().ok_or(super::LocalError::ProcessUncertain)?;
  if crate::run_processes::identity::capture(executor.pid)
    .map_err(|_| super::LocalError::ProcessUncertain)?
    .is_some_and(|current| current.started == executor.started)
  {
    return Err(super::LocalError::ProcessUncertain.into());
  }
  stopped(identity)
}
