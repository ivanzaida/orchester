//! The process group in place of the Windows job: every program [`crate::prepare`] builds leads a group of its own,
//! set in the child before it runs, so git and everything it starts are observed and stopped as one. Unlike the job,
//! a group does not end its processes when Orchester exits, and a process that leaves it on purpose (`setsid`) is
//! neither observed nor stopped with it (`docs/architecture/storage.md`).
use std::{
  path::PathBuf,
  process::{Child, Command},
  sync::OnceLock,
};

use orchester_core::{blob::ContentHash, id::IdempotencyKey, integration::local::ProcessIdentity};

use super::super::{LocalError, LocalGitError, digest};
use crate::process_group::ProcessGroup;

/// The group of the one program a family runs; none until it is started.
pub(in crate::local_git) struct Family {
  token: ContentHash,
  group: OnceLock<ProcessGroup>,
  /// The program the family started, which stands for its image once it exited.
  program: OnceLock<PathBuf>,
}
impl Family {
  pub(in crate::local_git) fn create() -> Result<Self, LocalGitError> {
    Ok(Self {
      token: digest(IdempotencyKey::generate().to_string().as_bytes()),
      group: OnceLock::new(),
      program: OnceLock::new(),
    })
  }
  pub(in crate::local_git) fn spawn(&self, command: &mut Command) -> Result<Child, LocalGitError> {
    if self.group.get().is_some() {
      return Err(LocalError::ProcessUncertain.into());
    }
    let mut child = command.spawn().map_err(|_| LocalGitError::Process)?;
    let Some(group) = ProcessGroup::led_by(child.id()) else {
      child.kill().map_err(|_| LocalError::ProcessUncertain)?;
      child.wait().map_err(|_| LocalError::ProcessUncertain)?;
      return Err(LocalError::ProcessUncertain.into());
    };
    self.group.set(group).map_err(|_| LocalError::ProcessUncertain)?;
    self
      .program
      .set(PathBuf::from(command.get_program()))
      .map_err(|_| LocalError::ProcessUncertain)?;
    Ok(child)
  }
  /// The child starts at once, unlike the job's suspended one, so a short program may have exited already: it is
  /// ours and not reaped yet, so its PID and start time still identify it.
  pub(in crate::local_git) fn identity(&self, child: &Child) -> Result<ProcessIdentity, LocalGitError> {
    let identity = match crate::run_processes::identity::capture(child.id()) {
      Ok(Some(identity)) => identity,
      Ok(None) => {
        let program = self.program.get().ok_or(LocalError::ProcessUncertain)?;
        crate::run_processes::identity::capture_exited_child(child.id(), program)
          .map_err(|_| LocalError::ProcessUncertain)?
          .ok_or(LocalError::ProcessUncertain)?
      }
      Err(_) => return Err(LocalError::ProcessUncertain.into()),
    };
    super::record(&self.token, identity)
  }
  /// The group was set before the program ran, which is what the job's suspended start makes sure of on Windows:
  /// nothing waits to be resumed.
  pub(in crate::local_git) fn resume(&self, _child: &Child) -> Result<(), LocalGitError> {
    Ok(())
  }
  /// Whether no process of the group is left; the leader counts until it is reaped.
  pub(in crate::local_git) fn empty(&self) -> Result<bool, LocalGitError> {
    self.group.get().map_or(Ok(true), |group| gone(*group))
  }
  pub(in crate::local_git) fn stop(&self) -> Result<(), LocalGitError> {
    match self.group.get() {
      Some(group) => group.kill().map_err(|_| LocalError::ProcessUncertain.into()),
      None => Ok(()),
    }
  }
}
/// Whether the group a family recorded in `identity` has no process left, e.g. after a restart. The group's id is its
/// leader's, which a later group may take once it is empty: a group found under that id counts as not stopped, so a
/// reused id never proves completion.
pub(in crate::local_git) fn stopped(identity: &ProcessIdentity) -> Result<bool, LocalGitError> {
  identity.containment.as_ref().ok_or(LocalError::ProcessUncertain)?;
  let group = ProcessGroup::led_by(identity.pid).ok_or(LocalError::ProcessUncertain)?;
  gone(group)
}
fn gone(group: ProcessGroup) -> Result<bool, LocalGitError> {
  group.is_gone().map_err(|_| LocalError::ProcessUncertain.into())
}
