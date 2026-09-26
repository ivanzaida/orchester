//! What must hold before a run starts (design: "Preflight must validate … available published
//! configuration, authorization, runner readiness"). Each problem is a typed, visible reason;
//! a run with any problem does not start.
//!
//! This version of the runner executes graphs of agent steps on the Codex and Claude Code CLIs —
//! parallel steps, failure routes and retries included; tool steps and approval gates are reported
//! as not supported yet rather than skipped.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  connection::{Connection, ConnectionKind, ConnectionStatus},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, Pipeline, StepKey, StepKind, Version},
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
  /// The profile's connection is not an agent CLI, so it cannot run the step.
  UnsupportedProvider { step: StepKey },
  /// The connection's last check says its CLI cannot be used (not signed in, missing, …).
  ConnectionNotUsable { step: StepKey, status: ConnectionStatus },
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
    if let Some(problem) = connection_problem(&connections, profile.body().connection(), key()) {
      problems.push(problem);
    }
  }
  problems
}

fn connection_problem(
  connections: &BTreeMap<ConnectionId, &Connection>,
  id: ConnectionId,
  step: StepKey,
) -> Option<PreflightProblem> {
  let Some(connection) = connections.get(&id) else {
    return Some(PreflightProblem::ConnectionMissing { step });
  };
  if !matches!(connection.kind(), ConnectionKind::Agent(_)) {
    return Some(PreflightProblem::UnsupportedProvider { step });
  }
  let status = connection.status();
  (!status.is_usable()).then_some(PreflightProblem::ConnectionNotUsable { step, status })
}
