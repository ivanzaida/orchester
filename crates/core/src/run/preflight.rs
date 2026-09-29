//! What must hold before a run starts (design: "Preflight must validate … available published
//! configuration, authorization, runner readiness"). Each problem is a typed, visible reason;
//! a run with any problem does not start.
//!
//! This version of the runner executes graphs of agent steps on the Codex and Claude Code CLIs and
//! of tool steps that run published command tools — parallel steps, failure routes, retries and
//! approval gates on agent steps (plan review) included. HTTP and database tools (secrets,
//! approvals), tools with access rules and approval gates on tool steps are reported as not
//! supported yet rather than skipped.

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
  connection::{AgentProvider, Connection, ConnectionKind, ConnectionStatus, invocation::Platform},
  id::{ConnectionId, ResourceId},
  permission::capability::PermissionProblem,
  resource::{AgentProfile, Pipeline, Step, StepKey, StepKind, Tool, ToolKind, Version},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "snake_case")]
pub enum PreflightProblem {
  /// The task has no pipeline, or its pipeline has no published version.
  PipelineNotPublished,
  /// A queued task predates version pinning, so its originally selected version is unknown.
  PipelineVersionNotPinned,
  /// A published pipeline has at least one step; this one has none.
  NoSteps,
  /// The step's tool has no published version.
  ToolNotPublished { step: StepKey },
  /// Tools of `kind` (HTTP, database) use project secrets and runtime approvals, which cannot be
  /// used yet.
  ToolNotRunnable { step: StepKey, kind: ToolKind },
  /// The step's command tool carries access rules; tool steps do not apply access rules yet.
  ToolAccessRules { step: StepKey },
  /// The step runs a tool and has an approval gate: only agent steps propose plans for review.
  ApprovalGate { step: StepKey },
  /// The step's agent profile has no published version.
  ProfileNotPublished { step: StepKey },
  /// The profile's connection no longer exists.
  ConnectionMissing { step: StepKey },
  /// The profile's connection is not an agent CLI, so it cannot run the step.
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
  /// The configured Cargo target's volume has too little space for a build.
  BuildSpaceLow {
    target: PathBuf,
    available: u64,
    required: u64,
  },
  /// The runner could not measure space on the configured Cargo target's volume.
  BuildSpaceUnavailable { target: PathBuf },
}

const GIB: u64 = 1024 * 1024 * 1024;

/// A new target can need 10–20 GiB; an existing target needs a smaller emergency floor so
/// incremental builds are not refused only because they have already filled their cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildTargetState {
  Empty,
  Populated,
}

/// The low-space problem for a shared Cargo target, if its host volume is below the floor.
pub fn build_space_problem(target: PathBuf, available: u64, state: BuildTargetState) -> Option<PreflightProblem> {
  let required = match state {
    BuildTargetState::Empty => 10 * GIB,
    BuildTargetState::Populated => GIB,
  };
  (available < required).then_some(PreflightProblem::BuildSpaceLow {
    target,
    available,
    required,
  })
}

/// The configuration a task's run would use.
#[derive(Clone, Copy, Debug)]
pub struct PreflightInput<'a> {
  /// The selected or pinned published version of the task's pipeline, if any.
  pub pipeline: Option<&'a Pipeline>,
  /// Latest published version of each agent profile.
  pub profiles: &'a BTreeMap<ResourceId, Version<AgentProfile>>,
  /// Latest published version of each tool.
  pub tools: &'a BTreeMap<ResourceId, Version<Tool>>,
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
    let profile = match &step.kind {
      StepKind::Tool { tool } => {
        if step.approval_gate {
          problems.push(PreflightProblem::ApprovalGate { step: key() });
        }
        let tool = tool.and_then(|tool| input.tools.get(&tool)).map(Version::body);
        problems.extend(tool_problem(tool, key()));
        continue;
      }
      StepKind::Agent { profile, .. } => profile.and_then(|profile| input.profiles.get(&profile)),
    };
    let Some(profile) = profile else {
      problems.push(PreflightProblem::ProfileNotPublished { step: key() });
      continue;
    };
    match usable_provider(&connections, profile.body().connection(), key()) {
      Ok(provider) => problems.extend(permission_problem(step, profile.body(), provider)),
      Err(problem) => problems.push(problem),
    }
  }
  problems
}

/// Why a tool step cannot run `tool` (its tool's latest published version), if it cannot.
fn tool_problem(tool: Option<&Tool>, step: StepKey) -> Option<PreflightProblem> {
  let Some(tool) = tool else {
    return Some(PreflightProblem::ToolNotPublished { step });
  };
  if !tool.kind().runnable() {
    return Some(PreflightProblem::ToolNotRunnable {
      step,
      kind: tool.kind(),
    });
  }
  tool.access().map(|_| PreflightProblem::ToolAccessRules { step })
}

/// Whether `provider`'s CLI enforces the step's effective permissions on this platform. Only
/// reached for steps on a usable agent connection.
fn permission_problem(step: &Step, profile: &AgentProfile, provider: AgentProvider) -> Option<PreflightProblem> {
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
    .enforced_on(provider, Platform::current())
    .err()?;
  Some(PreflightProblem::PermissionsNotEnforceable {
    step: step.key.clone(),
    reason,
  })
}

/// The agent CLI of connection `id`, or why the step cannot run on it.
fn usable_provider(
  connections: &BTreeMap<ConnectionId, &Connection>,
  id: ConnectionId,
  step: StepKey,
) -> Result<AgentProvider, PreflightProblem> {
  let Some(connection) = connections.get(&id) else {
    return Err(PreflightProblem::ConnectionMissing { step });
  };
  let ConnectionKind::Agent(provider) = connection.kind() else {
    return Err(PreflightProblem::UnsupportedProvider { step });
  };
  let status = connection.status();
  if !status.is_usable() {
    return Err(PreflightProblem::ConnectionNotUsable { step, status });
  }
  Ok(*provider)
}
