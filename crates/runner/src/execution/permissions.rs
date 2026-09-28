//! Permissions changed for a run (02.J). A change is recorded on the run and takes effect with
//! the next invocation of each step that uses the profile: the command of an invocation is built
//! from the run's permissions ([`orchester_core::run::Run::step_permissions`]). *Apply now* also
//! asks the profile's agents at work to stop once the command they are running has finished; each
//! one's work is committed as a checkpoint and its step continues from it as a new invocation
//! that resumes the CLI session with the new permissions (see `step::end`). Keeping the change
//! for the next invocation instead withdraws the stops that have not happened yet.

use orchester_core::{
  connection::{AgentProvider, ConnectionKind, invocation::Platform},
  id::{ResourceId, RunId},
  permission::AgentPermissions,
  resource::AgentProfile,
  run::{PermissionsApply, Run, RunError, StepExecutor},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{
  Scheduler,
  control::{Ack, Control, answer},
  failed, idle,
  record::{RecordError, record},
  run::{ExecutionError, RunTask},
  step::worker::{Stop, StopCause},
};

/// A person's change of one profile's permissions for a run.
#[derive(Clone, Debug)]
pub(super) struct PermissionsChange {
  pub(super) profile: ResourceId,
  pub(super) permissions: AgentPermissions,
  pub(super) apply: PermissionsApply,
}

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
      return change_idle(&mut self.open()?, run, &change);
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

impl RunTask {
  /// Records `change`; for Apply now, tells the profile's agents to stop after their command.
  pub(super) fn change_permissions(&mut self, change: PermissionsChange, ack: Ack) -> Result<(), ExecutionError> {
    let provider = match provider_of(&self.db, &self.run, change.profile) {
      Ok(provider) => provider,
      Err(rejection) => {
        answer(ack, Err(rejection));
        return Ok(());
      }
    };
    let recorded = record(&mut self.db, &self.notifier, &mut self.run, |run, now| {
      run.change_permissions(
        change.profile,
        change.permissions,
        change.apply,
        (provider, Platform::current()),
        now,
      )
    });
    if self.answered(recorded, ack)? {
      for (execution, active) in &self.agents {
        if self.run.restarts_for_permissions(*execution) && !active.abandoned {
          active.tell(Stop::AfterCommand(StopCause::Permissions));
        }
      }
    }
    Ok(())
  }

  /// Cancels the pending restarts of `profile`'s agents; stops that already happened stay, and
  /// their steps continue with the new permissions.
  pub(super) fn keep_permissions(&mut self, profile: ResourceId, ack: Ack) -> Result<(), ExecutionError> {
    let restarting: Vec<_> = self
      .run
      .changed_permissions(profile)
      .map(|changed| changed.restarts().iter().copied().collect())
      .unwrap_or_default();
    let recorded = record(&mut self.db, &self.notifier, &mut self.run, |run, now| {
      run.keep_permissions_for_next_invocation(profile, now)
    });
    if self.answered(recorded, ack)? {
      for execution in restarting {
        if let Some(active) = self.agents.get(&execution) {
          active.tell(Stop::Withdraw);
        }
      }
    }
    Ok(())
  }

  /// Answers a person's change: a change the run refuses is a rejection and the run goes on; a
  /// failure to store it ends the run's execution. `true` when it was recorded.
  fn answered(&self, recorded: Result<(), RecordError>, ack: Ack) -> Result<bool, ExecutionError> {
    match recorded {
      Ok(()) => {
        answer(ack, Ok(()));
        Ok(true)
      }
      Err(RecordError::Run(error)) => {
        answer(ack, Err(rejection(error)));
        Ok(false)
      }
      Err(error @ RecordError::Store(_)) => {
        answer(
          ack,
          Err(Rejection::Failed {
            detail: error.to_string(),
          }),
        );
        Err(error.into())
      }
    }
  }
}

/// A change for a run no task executes: it applies from its next invocations.
fn change_idle(db: &mut ProjectDb, run: RunId, change: &PermissionsChange) -> Result<(), Rejection> {
  let mut run = idle::load(db, run)?;
  let provider = provider_of(db, &run, change.profile)?;
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

/// The agent CLI of `profile`'s connection, as the run pinned the profile.
fn provider_of(db: &ProjectDb, run: &Run, profile: ResourceId) -> Result<AgentProvider, Rejection> {
  let pinned = run
    .snapshot()
    .steps()
    .iter()
    .find_map(|step| match step.executor() {
      StepExecutor::Agent { profile: pinned, .. } if pinned.resource == profile => Some(*pinned),
      _ => None,
    })
    .ok_or(Rejection::ProfileNotInRun)?;
  let version = db
    .version::<AgentProfile>(pinned.resource, pinned.number)
    .map_err(|error| failed("cannot read the agent profile", &error))?
    .ok_or(Rejection::ProfileNotInRun)?;
  let connection = db
    .connection(version.body().connection())
    .map_err(|error| failed("cannot read the profile's connection", &error))?;
  match connection.as_ref().map(|connection| connection.kind()) {
    Some(ConnectionKind::Agent(provider)) => Ok(*provider),
    _ => Err(Rejection::Failed {
      detail: "the agent profile's connection is not an agent CLI".to_owned(),
    }),
  }
}

/// What the desktop reads for a change the run refuses.
fn rejection(error: RunError) -> Rejection {
  match error {
    RunError::ProfileNotInRun(_) => Rejection::ProfileNotInRun,
    RunError::PermissionsNotEnforceable { step, problem } => Rejection::PermissionsNotEnforceable { step, problem },
    RunError::NoAgentToRestart(_) => Rejection::NoAgentToRestart,
    RunError::NoPendingPermissions(_) => Rejection::NoPendingPermissions,
    RunError::InvalidRunTransition { .. } => Rejection::RunEnded,
    other => idle::rejection(other),
  }
}
