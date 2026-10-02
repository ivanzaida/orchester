//! A step's permissions in its run, as the task page shows them and the run permissions popover
//! (02.J `q4K1G`) changes them: what its latest invocation launched with and what the next one runs
//! with, what the popover edits (the run's permissions for the step's profile, before the step
//! narrows them), what the profile itself allows, and whether a change waits for the next
//! invocation or, made with Apply now, is pending or applied (`LJmTH`).

use std::sync::Arc;

use orchester_core::{
  connection::{AgentProvider, invocation::Platform, stream::InvocationOutcome},
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
  /// The agent stopped at its checkpoint for the change, and the step has not launched again yet:
  /// it starts with the new permissions as soon as the run continues. Too late to keep the change
  /// for the next invocation instead.
  Restarting,
  /// The agent stopped at its checkpoint for the change, but the step's restart did not launch it;
  /// its activity says why.
  RestartFailed,
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
  /// What the step's current or last invocation actually launched with; never one whose CLI did not
  /// start.
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

/// What an invocation launched with, as the runner recorded it just before starting its CLI. Nothing
/// is inferred from the run's settings or event times.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvocationPolicy {
  /// Launched with these and still running.
  Active(AgentPermissions),
  /// Launched with these and ended.
  Last(AgentPermissions),
  /// Created, but its CLI has not launched yet (the runner prepares it first, e.g. the code index).
  Starting,
  /// Ended without a record: launched before Orchester recorded launches.
  Unknown,
  /// Its CLI never started; the activity says why.
  NotStarted,
  /// No invocation of the step has launched yet.
  NoInvocation,
}

impl InvocationPolicy {
  /// What `invocation` launched with.
  pub fn of(invocation: &Invocation) -> Self {
    if invocation.outcome == Some(InvocationOutcome::NotStarted) {
      return Self::NotStarted;
    }
    match (&invocation.permissions, invocation.ended_at) {
      (Some(launch), None) => Self::Active(launch.permissions().clone()),
      (Some(launch), Some(_)) => Self::Last(launch.permissions().clone()),
      (None, None) => Self::Starting,
      (None, Some(_)) => Self::Unknown,
    }
  }
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
  // Without a provider (the profile's connection cannot be read) `enforced` is `None`, so a run
  // change keeps reading as waiting for the next invocation: harmless, as nothing that could not be
  // checked is shown as active.
  let settled = state
    .as_ref()
    .is_none_or(|state| matches!(state, ChangeState::Applied { .. }));
  let state = if changed.is_some() && settled && !run.status().is_terminal() {
    queued(
      state,
      invocation,
      enforced.as_ref().map(EnforcedPermissions::permissions),
    )
  } else {
    state
  };
  Ok(Some(RunPermissions {
    invocation_policy: invocation.map_or(InvocationPolicy::NoInvocation, InvocationPolicy::of),
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

/// The step's latest invocation, of any of its executions, leaving out those whose CLI never
/// started: they launched with nothing, so the row keeps showing the one before.
fn latest_invocation<'a>(run: &Run, invocations: &'a [Invocation], step: &StepKey) -> Option<&'a Invocation> {
  invocations.iter().rev().find(|invocation| {
    invocation.outcome != Some(InvocationOutcome::NotStarted)
      && run
        .execution(invocation.execution)
        .is_some_and(|execution| execution.step() == step)
  })
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

/// Pending while an agent of the profile has yet to stop at its checkpoint; restarting until the
/// step's new invocation has launched (or failed to); applied while the step's latest execution is
/// the one that restarted and the run has not ended.
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
  // The agent restarts right after the change is recorded as applied; it counts as applied once
  // its new invocation has recorded what it launched with.
  let restarts = || {
    invocations
      .iter()
      .filter(|invocation| invocation.execution == applied.execution && invocation.started_at >= applied.at)
  };
  if let Some(invocation) = restarts().find(|invocation| invocation.permissions.is_some()) {
    return Ok(Some(ChangeState::Applied {
      at: applied.at,
      checkpoint,
      invocation: invocation.number,
    }));
  }
  let failed = restarts().any(|invocation| invocation.outcome == Some(InvocationOutcome::NotStarted));
  Ok(Some(if failed {
    ChangeState::RestartFailed
  } else {
    ChangeState::Restarting
  }))
}
