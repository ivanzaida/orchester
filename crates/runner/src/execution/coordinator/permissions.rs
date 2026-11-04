//! A person's change of a profile's permissions for a run (`crate::execution::executor::permissions`), as the
//! scheduler takes it: the task executing the run records it, or, while none does, it is recorded here and applies
//! from the run's next invocations.

use orchester_core::{
  connection::invocation::Platform,
  id::{ResourceId, RunId},
  permission::AgentPermissions,
  run::PermissionsApply,
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{Scheduler, idle, link::LocalLink};
use crate::{
  computer::ComputerResources,
  execution::executor::{
    control::Control,
    permissions::{PermissionsChange, provider_of, rejection},
  },
};

impl Scheduler {
  /// Changes the permissions of agent profile `profile` for `run`. Returns once the change is
  /// recorded; with [`PermissionsApply::Now`] the profile's agents at work restart later, at their
  /// next checkpoint.
  pub async fn change_permissions(
    &self,
    run: RunId,
    profile: ResourceId,
    permissions: AgentPermissions,
    apply: PermissionsApply,
  ) -> Result<(), Rejection> {
    let change = PermissionsChange {
      profile,
      permissions,
      apply,
    };
    loop {
      let one_at_a_time = self.inner.continuing.lock().await;
      if self.inner.board.is_stopping() {
        return Err(Rejection::ShuttingDown);
      }
      if let Some(answer) = self.ask(run, |ack| Control::ChangePermissions(change.clone(), ack)) {
        drop(one_at_a_time);
        if let Ok(result) = answer.await {
          return result;
        }
        continue;
      }
      return change_idle((&mut self.open()?, &*self.inner.computer), run, &change);
    }
  }

  /// Keeps the pending change of `profile`'s permissions in `run` for the next invocations instead
  /// of restarting its agents now.
  pub async fn keep_permissions_for_next_invocation(&self, run: RunId, profile: ResourceId) -> Result<(), Rejection> {
    loop {
      let one_at_a_time = self.inner.continuing.lock().await;
      if self.inner.board.is_stopping() {
        return Err(Rejection::ShuttingDown);
      }
      if let Some(answer) = self.ask(run, |ack| Control::KeepPermissions(profile, ack)) {
        drop(one_at_a_time);
        if let Ok(result) = answer.await {
          return result;
        }
        continue;
      }
      return keep_idle(&mut self.open()?, run, profile);
    }
  }
}

/// A change for a run no task executes: it applies from its next invocations; its profile's connection is read on
/// `computer`, where the run executes.
fn change_idle(
  (db, computer): (&mut ProjectDb, &dyn ComputerResources),
  run: RunId,
  change: &PermissionsChange,
) -> Result<(), Rejection> {
  let mut run = idle::load(db, run)?;
  let provider = provider_of((&LocalLink::over(&*db), computer), &run, change.profile)?;
  let events = run
    .change_permissions(
      change.profile,
      change.permissions.clone(),
      change.apply,
      (provider, Platform::current()),
      Timestamp::now(),
    )
    .map_err(rejection)?;
  idle::store(db, &run, &events)
}

fn keep_idle(db: &mut ProjectDb, run: RunId, profile: ResourceId) -> Result<(), Rejection> {
  let mut run = idle::load(db, run)?;
  let events = run
    .keep_permissions_for_next_invocation(profile, Timestamp::now())
    .map_err(rejection)?;
  idle::store(db, &run, &events)
}
