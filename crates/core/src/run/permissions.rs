//! Permissions changed for one run (02.J, `docs/design/orchester-workspace.md`, "Change for this
//! run"). A change stands in for the permissions of one agent profile in this run only: every step
//! of the run that uses the profile gets them, and each step still narrows them as it narrows the
//! profile's. The published profile keeps its permissions.
//!
//! A change applies from the next invocation of each of those steps. *Apply now* also restarts the
//! profile's agents at work: each finishes the command it is running, its work is committed as a
//! checkpoint and its step continues from there as a new invocation (the CLI session resumed) with
//! the new permissions. Until an agent has restarted the change is pending, and keeping it for the
//! next invocation instead cancels the restarts. A change the connection's CLI cannot enforce is
//! refused, as publishing refuses it.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{
  error::RunError,
  event::{RunEvent, RunEventKind},
  snapshot::StepExecutor,
  state::Run,
  status::{RunStatus, StepStatus},
};
use crate::{
  connection::{AgentProvider, invocation::Platform},
  id::{CheckpointId, ResourceId, StepExecutionId},
  permission::AgentPermissions,
  resource::{StepKey, StepKind},
  time::Timestamp,
};

/// When changed permissions take effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionsApply {
  /// From the next invocation of each step; invocations running now keep theirs.
  NextInvocation,
  /// Also restart the agents at work at their next checkpoint.
  Now,
}

/// Who changed a run's permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionsChangedBy {
  /// A person, in the run permissions popover.
  User,
}

/// An agent that restarted with changed permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedPermissions {
  pub execution: StepExecutionId,
  /// The checkpoint its work was saved in before it restarted; `None` when nothing had changed.
  pub checkpoint: Option<CheckpointId>,
  pub at: Timestamp,
}

/// The permissions a run uses for one agent profile instead of the profile's own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPermissions {
  permissions: AgentPermissions,
  apply: PermissionsApply,
  by: PermissionsChangedBy,
  changed_at: Timestamp,
  /// Agents to restart with them at their next checkpoint (*Apply now*), until each has.
  restarts: BTreeSet<StepExecutionId>,
  /// The last agent that restarted with them.
  applied: Option<AppliedPermissions>,
}

impl RunPermissions {
  pub const fn permissions(&self) -> &AgentPermissions {
    &self.permissions
  }

  pub const fn apply(&self) -> PermissionsApply {
    self.apply
  }

  pub const fn changed_by(&self) -> PermissionsChangedBy {
    self.by
  }

  pub const fn changed_at(&self) -> Timestamp {
    self.changed_at
  }

  /// The agents that restart with these permissions at their next checkpoint.
  pub const fn restarts(&self) -> &BTreeSet<StepExecutionId> {
    &self.restarts
  }

  /// "Applying new permissions at the next checkpoint…".
  pub fn is_pending(&self) -> bool {
    !self.restarts.is_empty()
  }

  pub const fn applied(&self) -> Option<AppliedPermissions> {
    self.applied
  }
}

impl Run {
  /// Changes the permissions of agent profile `profile` for this run to `permissions`, which its
  /// connection's CLI (`provider` on `platform`) must enforce for every step of the run that uses
  /// the profile, as each step narrows them. [`PermissionsApply::Now`] needs an agent of the
  /// profile at work, which then restarts at its next checkpoint.
  pub fn change_permissions(
    &mut self,
    profile: ResourceId,
    permissions: AgentPermissions,
    apply: PermissionsApply,
    (provider, platform): (AgentProvider, Platform),
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    let steps = self.steps_of_profile(profile);
    for step in &steps {
      let (repository, narrowing) = match &self.snapshot().body().step(step).map(|step| &step.kind) {
        Some(StepKind::Agent {
          repository,
          permissions,
          ..
        }) => (*repository, permissions.clone()),
        _ => continue,
      };
      permissions
        .narrowed(repository, &narrowing)
        .enforced_on(provider, platform)
        .map_err(|problem| RunError::PermissionsNotEnforceable {
          step: step.clone(),
          problem,
        })?;
    }
    self.transition(now, |transition| {
      transition.record(RunEventKind::PermissionsChanged {
        profile,
        permissions,
        apply,
        by: PermissionsChangedBy::User,
      })
    })
  }

  /// Keeps the pending change of `profile`'s permissions for the next invocations instead of
  /// restarting its agents now ("Keep for next invocation instead").
  pub fn keep_permissions_for_next_invocation(
    &mut self,
    profile: ResourceId,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::PermissionsRestartCancelled { profile })
    })
  }

  /// Records that `execution`'s agent ended at a checkpoint to take changed permissions
  /// (`checkpoint`, if its work changed anything) and continues with them.
  pub fn apply_permissions(
    &mut self,
    execution: StepExecutionId,
    checkpoint: Option<CheckpointId>,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::PermissionsApplied { execution, checkpoint })
    })
  }

  /// The permissions this run uses for `profile` instead of the profile's, if they were changed.
  pub fn changed_permissions(&self, profile: ResourceId) -> Option<&RunPermissions> {
    self.permissions.get(&profile)
  }

  /// The permissions an invocation of `step` runs with: the run's for its profile, else
  /// `profile_permissions` (the pinned profile version's), narrowed by the step.
  pub fn step_permissions(&self, step: &StepKey, profile_permissions: &AgentPermissions) -> Option<AgentPermissions> {
    let pipeline_step = self.snapshot().body().step(step)?;
    let StepKind::Agent {
      repository,
      permissions: narrowing,
      ..
    } = &pipeline_step.kind
    else {
      return None;
    };
    let base = self
      .profile_of(step)
      .and_then(|profile| self.changed_permissions(profile))
      .map_or(profile_permissions, RunPermissions::permissions);
    Some(base.narrowed(*repository, narrowing))
  }

  /// Whether `execution`'s agent is to restart with changed permissions at its next checkpoint.
  pub fn restarts_for_permissions(&self, execution: StepExecutionId) -> bool {
    self
      .permissions
      .values()
      .any(|changed| changed.restarts.contains(&execution))
  }

  /// Whether an agent of `profile` is at work in a running run that is not waiting to pause, so
  /// Apply now can restart it.
  pub fn can_apply_permissions_now(&self, profile: ResourceId) -> bool {
    self.status == RunStatus::Running && !self.pause_requested && !self.running_of_profile(profile).is_empty()
  }

  /// The agent profile `step` runs with in this run.
  pub fn profile_of(&self, step: &StepKey) -> Option<ResourceId> {
    match self.snapshot().step(step)?.executor() {
      StepExecutor::Agent { profile, .. } => Some(profile.resource),
      StepExecutor::Tool { .. } => None,
    }
  }

  fn steps_of_profile(&self, profile: ResourceId) -> Vec<StepKey> {
    self
      .snapshot()
      .steps()
      .iter()
      .filter(|step| self.profile_of(step.key()) == Some(profile))
      .map(|step| step.key().clone())
      .collect()
  }

  /// Executions of the current attempt at work on a step of `profile`.
  fn running_of_profile(&self, profile: ResourceId) -> BTreeSet<StepExecutionId> {
    self
      .running_executions()
      .into_iter()
      .filter(|id| {
        self
          .execution(*id)
          .is_some_and(|execution| self.profile_of(execution.step()) == Some(profile))
      })
      .collect()
  }

  pub(super) fn check_permissions_change(
    &self,
    kind: &RunEventKind,
    profile: ResourceId,
    apply: PermissionsApply,
  ) -> Result<(), RunError> {
    if self.status.is_terminal() {
      return Err(RunError::InvalidRunTransition {
        status: self.status.clone(),
        event: kind.name(),
      });
    }
    if self.steps_of_profile(profile).is_empty() {
      return Err(RunError::ProfileNotInRun(profile));
    }
    if apply == PermissionsApply::Now && !self.can_apply_permissions_now(profile) {
      return Err(RunError::NoAgentToRestart(profile));
    }
    Ok(())
  }

  pub(super) fn check_restart_cancel(&self, profile: ResourceId) -> Result<(), RunError> {
    match self.permissions.get(&profile) {
      Some(changed) if changed.is_pending() => Ok(()),
      _ => Err(RunError::NoPendingPermissions(profile)),
    }
  }

  pub(super) fn check_permissions_applied(&self, execution: StepExecutionId) -> Result<(), RunError> {
    if self.restarts_for_permissions(execution) {
      Ok(())
    } else {
      Err(RunError::NotRestartingForPermissions(execution))
    }
  }

  pub(super) fn apply_permissions_change(
    &mut self,
    profile: ResourceId,
    (permissions, apply, by): (&AgentPermissions, PermissionsApply, PermissionsChangedBy),
    at: Timestamp,
  ) {
    let restarts = match apply {
      PermissionsApply::Now => self.running_of_profile(profile),
      PermissionsApply::NextInvocation => BTreeSet::new(),
    };
    let applied = self.permissions.get(&profile).and_then(RunPermissions::applied);
    self.permissions.insert(
      profile,
      RunPermissions {
        permissions: permissions.clone(),
        apply,
        by,
        changed_at: at,
        restarts,
        applied,
      },
    );
  }

  pub(super) fn apply_restart_cancel(&mut self, profile: ResourceId) {
    if let Some(changed) = self.permissions.get_mut(&profile) {
      changed.restarts.clear();
    }
  }

  pub(super) fn apply_permissions_applied(
    &mut self,
    execution: StepExecutionId,
    checkpoint: Option<CheckpointId>,
    at: Timestamp,
  ) {
    for changed in self.permissions.values_mut() {
      if changed.restarts.remove(&execution) {
        changed.applied = Some(AppliedPermissions {
          execution,
          checkpoint,
          at,
        });
      }
    }
  }

  /// A restart is only pending for an agent at work in a running run: once its execution or the
  /// run stopped, its next invocation takes the changed permissions anyway.
  pub(super) fn drop_stale_restarts(&mut self) {
    let running = self.status == RunStatus::Running;
    let open: BTreeSet<StepExecutionId> = if running {
      self.running_executions().into_iter().collect()
    } else {
      BTreeSet::new()
    };
    for changed in self.permissions.values_mut() {
      changed.restarts.retain(|execution| open.contains(execution));
    }
  }

  /// Changed permissions a restored run holds must name its own profiles and executions.
  pub(super) fn check_restored_permissions(&self) -> Result<(), RunError> {
    for (profile, changed) in &self.permissions {
      if self.steps_of_profile(*profile).is_empty() {
        return Err(RunError::Inconsistent(
          "changed permissions name a profile outside the run",
        ));
      }
      let known = changed.restarts.iter().all(|execution| {
        self
          .execution(*execution)
          .is_some_and(|execution| execution.status() == StepStatus::Running)
      });
      if !known {
        return Err(RunError::Inconsistent(
          "a permission restart names an execution not at work",
        ));
      }
    }
    Ok(())
  }
}
