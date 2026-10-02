//! A step's permissions in its run, as the task page shows them and the run permissions popover
//! (02.J `q4K1G`) changes them: what its latest invocation launched with and what the next one runs
//! with, what the popover edits (the run's permissions for the step's profile, before the step
//! narrows them), what the profile itself allows, and whether a change waits for the next
//! invocation or, made with Apply now, is pending or applied (`LJmTH`).

use std::sync::Arc;

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  id::ResourceId,
  permission::{AgentPermissions, LaunchPermissions, capability::EnforcedPermissions},
  resource::{AgentProfile, StepKey, StepKind, Version, VersionNumber},
  revision::Revision,
  run::{Invocation, Run, RunEventKind},
  store::{McpServerStore, ResourceStore, RunStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::detail::run_store_error;

/// Where a change of the run's permissions stands: kept for the next invocation, or made with Apply now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeState {
  /// Changed for the run, but no invocation has launched with the change yet: it takes effect when
  /// the agent next starts.
  NextInvocation,
  /// "Applying new permissions at the next checkpoint…": the agent finishes its command first.
  Pending,
  /// "New permissions active since 12:09": the agent restarted from `checkpoint` (when its work had
  /// changed anything) as invocation `invocation`.
  Applied {
    at: Timestamp,
    checkpoint: Option<Revision>,
    invocation: u32,
  },
}

/// A step's permissions in its run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunPermissions {
  /// What its next invocation runs with: [`Self::base`] narrowed by the step.
  pub permissions: AgentPermissions,
  /// What the step's current or last invocation actually launched with.
  pub invocation_policy: InvocationPolicy,
  /// Network off was chosen, but the CLI cannot enforce it, so network stays on.
  pub network_stays_on: bool,
  /// The listed commands a native agent runs unconfined (core's
  /// [`orchester_core::permission::EnforcedPermissions::unconfined_list`]); empty otherwise.
  pub unconfined_commands: Vec<Arc<str>>,
  /// The profile version the permissions come from.
  pub profile: VersionNumber,
  pub profile_id: ResourceId,
  /// The permissions the run uses for the profile: changed for the run, else the profile's.
  pub base: AgentPermissions,
  /// The pinned profile version's own permissions.
  pub profile_permissions: AgentPermissions,
  /// The permissions were changed for this run.
  pub changed: bool,
  pub state: Option<ChangeState>,
  /// An agent of the profile is at work, so Apply now can restart it.
  pub can_apply_now: bool,
  /// The version the profile's draft publishes as ("draft v3").
  pub draft: u32,
  /// The MCP servers the pinned profile version gives its agents, by name (by id when the project
  /// no longer has one). They are not changed for a run.
  pub mcp_servers: Vec<Arc<str>>,
  pub tool_approvals_required: bool,
}

/// The permissions the step's latest invocation launched with, as the runner recorded them before
/// starting it. `None` for an invocation recorded before launches were: its permissions stay unknown
/// rather than being inferred from the run's settings or event times.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvocationPolicy {
  /// The invocation is still running.
  Active(Option<AgentPermissions>),
  /// The invocation has ended.
  Last(Option<AgentPermissions>),
  /// No invocation of the step has launched yet.
  NotStarted,
}

/// What the run's step `step`, on the pinned profile version `version` of `profile_id`, runs with.
pub fn run_permissions(
  db: &ProjectDb,
  (run, invocations): (&Run, &[Invocation]),
  step: &StepKey,
  (profile_id, version): (ResourceId, &Version<AgentProfile>),
  provider: Option<AgentProvider>,
) -> Result<Option<RunPermissions>, StoreError> {
  let profile_permissions = version.body().permissions().clone();
  let Some(permissions) = run.step_permissions(step, &profile_permissions) else {
    return Ok(None);
  };
  let changed = run.changed_permissions(profile_id);
  let base = changed.map_or_else(|| profile_permissions.clone(), |changed| changed.permissions().clone());
  // Permissions the CLI cannot enforce at all never ran (preflight and the runner refuse them), so
  // they read as chosen.
  let enforced = provider.and_then(|provider| permissions.enforced_on(provider, Platform::current()).ok());
  let network_stays_on = enforced
    .as_ref()
    .is_some_and(|enforced| enforced.network_stays_on().is_some());
  let unconfined_commands = enforced
    .as_ref()
    .and_then(|enforced| enforced.unconfined_list())
    .map(|list| list.entries().iter().map(|entry| Arc::from(entry.as_str())).collect())
    .unwrap_or_default();
  let latest = db
    .latest_version::<AgentProfile>(profile_id)?
    .map_or(version.number(), |latest| latest.number());
  let invocation = latest_invocation(run, invocations, step);
  let state = change_state(db, (run, invocations), step, profile_id)?;
  let state = if changed.is_some() && state != Some(ChangeState::Pending) && !run.status().is_terminal() {
    queued(
      state,
      invocation,
      enforced.as_ref().map(EnforcedPermissions::permissions),
    )
  } else {
    state
  };
  Ok(Some(RunPermissions {
    invocation_policy: invocation_policy(invocation),
    permissions,
    network_stays_on,
    unconfined_commands,
    profile: version.number(),
    profile_id,
    base,
    profile_permissions,
    changed: changed.is_some(),
    state,
    can_apply_now: run.can_apply_permissions_now(profile_id),
    draft: latest.get().saturating_add(1),
    mcp_servers: mcp_servers(db, version.body())?,
    tool_approvals_required: matches!(run.snapshot().body().step(step).map(|step| &step.kind),
      Some(StepKind::Agent { permissions, .. }) if permissions.interactive_tool_approvals),
  }))
}

/// The step's latest invocation, of its latest execution.
fn latest_invocation<'a>(run: &Run, invocations: &'a [Invocation], step: &StepKey) -> Option<&'a Invocation> {
  let execution = run.latest_execution(step)?.id();
  invocations
    .iter()
    .rev()
    .find(|invocation| invocation.execution == execution)
}

fn invocation_policy(invocation: Option<&Invocation>) -> InvocationPolicy {
  let Some(invocation) = invocation else {
    return InvocationPolicy::NotStarted;
  };
  let launched = invocation
    .permissions
    .as_ref()
    .map(|launch| launch.permissions().clone());
  if invocation.ended_at.is_none() {
    InvocationPolicy::Active(launched)
  } else {
    InvocationPolicy::Last(launched)
  }
}

/// A run change reads as waiting for the next invocation until an invocation has launched with
/// exactly the permissions it enforces (`expected`); one whose launch was not recorded, or whose
/// desired settings cannot be enforced at all, never counts as having launched with it.
fn queued(
  state: Option<ChangeState>,
  invocation: Option<&Invocation>,
  expected: Option<&AgentPermissions>,
) -> Option<ChangeState> {
  let launched = invocation
    .and_then(|invocation| invocation.permissions.as_ref())
    .map(LaunchPermissions::permissions);
  match (launched, expected) {
    (Some(launched), Some(expected)) if launched == expected => state,
    _ => Some(ChangeState::NextInvocation),
  }
}

/// The names of the MCP servers `profile` chooses, in its order.
fn mcp_servers(db: &ProjectDb, profile: &AgentProfile) -> Result<Vec<Arc<str>>, StoreError> {
  profile
    .mcp_servers()
    .ids()
    .iter()
    .map(|id| {
      Ok(
        db.mcp_server(*id)?
          .map_or_else(|| id.to_string().into(), |server| server.name().as_str().into()),
      )
    })
    .collect()
}

/// Pending while an agent of the profile waits to restart or its new invocation has not started;
/// applied while the step's latest execution is the one that restarted and the run has not ended.
fn change_state(
  db: &ProjectDb,
  (run, invocations): (&Run, &[Invocation]),
  step: &StepKey,
  profile: ResourceId,
) -> Result<Option<ChangeState>, StoreError> {
  let Some(changed) = run.changed_permissions(profile) else {
    return Ok(None);
  };
  if changed.is_pending() {
    return Ok(Some(ChangeState::Pending));
  }
  let Some(applied) = changed.applied() else {
    return Ok(None);
  };
  // A later change for the next invocation replaces the one applied now.
  if changed.changed_at() > applied.at {
    return Ok(None);
  }
  let current = run.latest_execution(step).map(|execution| execution.id());
  if run.status().is_terminal() || current != Some(applied.execution) {
    return Ok(None);
  }
  let checkpoint = match applied.checkpoint {
    Some(checkpoint) => db
      .events(run.id())
      .map_err(run_store_error)?
      .into_iter()
      .find_map(|event| match event.kind() {
        RunEventKind::CheckpointRecorded {
          checkpoint: recorded,
          revision,
          ..
        } if *recorded == checkpoint => Some(revision.clone()),
        _ => None,
      }),
    None => None,
  };
  // The agent restarts right after the change is recorded as applied; until its new invocation
  // has started and recorded what it launched with, the change still reads as being applied.
  let Some(invocation) = invocations
    .iter()
    .find(|invocation| {
      invocation.execution == applied.execution
        && invocation.started_at >= applied.at
        && invocation.permissions.is_some()
    })
    .map(|invocation| invocation.number)
  else {
    return Ok(Some(ChangeState::Pending));
  };
  Ok(Some(ChangeState::Applied {
    at: applied.at,
    checkpoint,
    invocation,
  }))
}
