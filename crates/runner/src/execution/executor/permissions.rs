//! Permissions changed for a run (02.J). A change is recorded on the run and takes effect with
//! the next invocation of each step that uses the profile: the command of an invocation is built
//! from the run's permissions ([`orchester_core::run::Run::step_permissions`]). *Apply now* also
//! asks the profile's agents at work to stop once the command they are running has finished; each
//! one's work is committed as a checkpoint and its step continues from it as a new invocation
//! that resumes the CLI session with the new permissions (see `step::end`). Keeping the change
//! for the next invocation instead withdraws the stops that have not happened yet. The scheduler's
//! side of the request is `crate::execution::coordinator::permissions`'s.

use orchester_core::{
  connection::{AgentProvider, ConnectionKind, invocation::Platform},
  id::ResourceId,
  permission::AgentPermissions,
  resource::AgentProfile,
  run::{PermissionsApply, Run, RunError, StepExecutor},
};
use orchester_ipc::Rejection;

use super::{
  control::{Ack, answer},
  failed,
  link::resources::{ResourceLink, pinned_version},
  record::{RecordError, record},
  run::{ExecutionError, RunTask},
  step::worker::{Stop, StopCause},
};
use crate::{computer::ComputerResources, execution::coordinator::idle};

/// A person's change of one profile's permissions for a run.
#[derive(Clone, Debug)]
pub(in crate::execution) struct PermissionsChange {
  pub(in crate::execution) profile: ResourceId,
  pub(in crate::execution) permissions: AgentPermissions,
  pub(in crate::execution) apply: PermissionsApply,
}

impl RunTask {
  /// Records `change`; for Apply now, tells the profile's agents to stop after their command.
  pub(super) fn change_permissions(&mut self, change: PermissionsChange, ack: Ack) -> Result<(), ExecutionError> {
    let provider = match provider_of((&*self.link, &*self.computer), &self.run, change.profile) {
      Ok(provider) => provider,
      Err(rejection) => {
        answer(ack, Err(rejection));
        return Ok(());
      }
    };
    let recorded = record(self.sink.as_mut(), &self.notifier, &mut self.run, |run, now| {
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
    let recorded = record(self.sink.as_mut(), &self.notifier, &mut self.run, |run, now| {
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

/// The agent CLI of `profile`'s connection, as the run pinned the profile: the profile version asked through `link`,
/// its connection read on `computer`, where the run executes.
pub(in crate::execution) fn provider_of(
  (link, computer): (&(impl ResourceLink + ?Sized), &dyn ComputerResources),
  run: &Run,
  profile: ResourceId,
) -> Result<AgentProvider, Rejection> {
  let pinned = run
    .snapshot()
    .steps()
    .iter()
    .find_map(|step| match step.executor() {
      StepExecutor::Agent { profile: pinned, .. } if pinned.resource == profile => Some(*pinned),
      _ => None,
    })
    .ok_or(Rejection::ProfileNotInRun)?;
  let body = pinned_version::<AgentProfile>(link, &pinned)
    .map_err(|error| failed("cannot read the agent profile", &error))?
    .ok_or(Rejection::ProfileNotInRun)?;
  let connection = computer
    .connection(body.connection())
    .map_err(|error| failed("cannot read the profile's connection", &error))?;
  match connection.as_ref().map(|connection| connection.connection().kind()) {
    Some(ConnectionKind::Agent(provider)) => Ok(*provider),
    _ => Err(Rejection::Failed {
      detail: "the agent profile's connection is not an agent CLI".to_owned(),
    }),
  }
}

/// What the desktop reads for a change the run refuses.
pub(in crate::execution) fn rejection(error: RunError) -> Rejection {
  match error {
    RunError::ProfileNotInRun(_) => Rejection::ProfileNotInRun,
    RunError::PermissionsNotEnforceable { step, problem } => Rejection::PermissionsNotEnforceable { step, problem },
    RunError::NoAgentToRestart(_) => Rejection::NoAgentToRestart,
    RunError::NoPendingPermissions(_) => Rejection::NoPendingPermissions,
    RunError::InvalidRunTransition { .. } => Rejection::RunEnded,
    other => idle::rejection(other),
  }
}
