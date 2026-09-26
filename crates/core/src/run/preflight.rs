//! What must hold before a run starts (design: "Preflight must validate … available published
//! configuration, authorization, runner readiness"). Each problem is a typed, visible reason;
//! a run with any problem does not start.
//!
//! This version of the runner executes graphs of agent steps on the Codex CLI — parallel steps,
//! failure routes and retries included; tool steps, approval gates and other providers are
//! reported as not supported yet rather than skipped.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  connection::{AgentProvider, Connection, ConnectionKind, ConnectionStatus, invocation::Platform},
  id::{ConnectionId, ResourceId},
  permission::capability::PermissionProblem,
  resource::{AgentProfile, Pipeline, Step, StepKey, StepKind, Version},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "snake_case")]
pub enum PreflightProblem {
  /// The task has no pipeline, or its pipeline has no published version.
  PipelineNotPublished,
  /// A published pipeline has at least one step; this one has none.
  NoSteps,
  /// Tool steps cannot run yet.
  ToolStep { step: StepKey },
  /// Approval gates (plan approval) cannot be answered yet.
  ApprovalGate { step: StepKey },
  /// The step's agent profile has no published version.
  ProfileNotPublished { step: StepKey },
  /// The profile's connection no longer exists.
  ConnectionMissing { step: StepKey },
  /// The profile runs on a provider other than the Codex CLI, which cannot run yet.
  UnsupportedProvider { step: StepKey },
  /// The connection's last check says its CLI cannot be used (not signed in, missing, …).
  ConnectionNotUsable { step: StepKey, status: ConnectionStatus },
  /// The connection's CLI cannot enforce the step's effective permissions (profile ∩ step) on this
  /// platform; a step never runs with permissions that are not enforced.
  PermissionsNotEnforceable { step: StepKey, reason: PermissionProblem },
  /// The project folder is not a git repository the runner can branch from.
  NotAGitRepository,
  /// The repository has no commit to start the run's branch from.
  NoCommits,
}

/// The configuration a task's run would use.
#[derive(Clone, Copy, Debug)]
pub struct PreflightInput<'a> {
  /// The latest published version of the task's pipeline, if any.
  pub pipeline: Option<&'a Pipeline>,
  /// Latest published version of each agent profile.
  pub profiles: &'a BTreeMap<ResourceId, Version<AgentProfile>>,
  pub connections: &'a [Connection],
}

/// Every configuration problem that keeps the run from starting, in step order. Repository
/// problems are found by the runner, which owns the git checkout.
pub fn preflight(input: PreflightInput<'_>) -> Vec<PreflightProblem> {
  let Some(pipeline) = input.pipeline else {
    return vec![PreflightProblem::PipelineNotPublished];
  };
  if pipeline.steps().is_empty() {
    return vec![PreflightProblem::NoSteps];
  }
  let connections: BTreeMap<ConnectionId, &Connection> = input
    .connections
    .iter()
    .map(|connection| (connection.id(), connection))
    .collect();
  let mut problems = Vec::new();
  for step in pipeline.steps() {
    let key = || step.key.clone();
    if step.approval_gate {
      problems.push(PreflightProblem::ApprovalGate { step: key() });
    }
    let profile = match &step.kind {
      StepKind::Tool { .. } => {
        problems.push(PreflightProblem::ToolStep { step: key() });
        continue;
      }
      StepKind::Agent { profile, .. } => profile.and_then(|profile| input.profiles.get(&profile)),
    };
    let Some(profile) = profile else {
      problems.push(PreflightProblem::ProfileNotPublished { step: key() });
      continue;
    };
    match connection_problem(&connections, profile.body().connection(), key()) {
      Some(problem) => problems.push(problem),
      None => problems.extend(permission_problem(step, profile.body())),
    }
  }
  problems
}

/// Whether the Codex CLI enforces the step's effective permissions on this platform. Only
/// reached for steps on a usable Codex connection.
fn permission_problem(step: &Step, profile: &AgentProfile) -> Option<PreflightProblem> {
  let StepKind::Agent {
    repository,
    permissions,
    ..
  } = &step.kind
  else {
    return None;
  };
  let reason = profile
    .permissions()
    .narrowed(*repository, permissions)
    .enforced_on(AgentProvider::CodexCli, Platform::current())
    .err()?;
  Some(PreflightProblem::PermissionsNotEnforceable {
    step: step.key.clone(),
    reason,
  })
}

fn connection_problem(
  connections: &BTreeMap<ConnectionId, &Connection>,
  id: ConnectionId,
  step: StepKey,
) -> Option<PreflightProblem> {
  let Some(connection) = connections.get(&id) else {
    return Some(PreflightProblem::ConnectionMissing { step });
  };
  if connection.kind() != &ConnectionKind::Agent(AgentProvider::CodexCli) {
    return Some(PreflightProblem::UnsupportedProvider { step });
  }
  let status = connection.status();
  (!status.is_usable()).then_some(PreflightProblem::ConnectionNotUsable { step, status })
}
