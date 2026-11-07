//! A pipeline as the team server holds it (`docs/design/team-configuration.md`, "Per resource kind"): each step runs an
//! agent profile or calls a tool of the team, and the Deliver step's integrator is a profile of the team, every one
//! named by its id on the server. A team resource keeps the server's id on every computer, so the body needs no
//! rewriting; what the team body must rule out is an id of a resource the server does not have, such as a profile only
//! this computer keeps ([`Pipeline::check_team`]). A local pipeline keeps naming its own profiles and tools.
//!
//! The pipeline's `references` stay empty: the server reads what a pipeline names from these body fields
//! (`docs/cloud-api.md`, used-by), so the body is the one source of truth.

use std::collections::HashSet;

use thiserror::Error;

use super::{Pipeline, StepKey, StepKind};
use crate::id::ResourceId;

/// The agent profiles and tools of a team project, by their ids on the server: what a team pipeline may name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamExecutors {
  profiles: HashSet<ResourceId>,
  tools: HashSet<ResourceId>,
}

impl TeamExecutors {
  pub fn new(profiles: impl IntoIterator<Item = ResourceId>, tools: impl IntoIterator<Item = ResourceId>) -> Self {
    Self {
      profiles: profiles.into_iter().collect(),
      tools: tools.into_iter().collect(),
    }
  }
}

/// Why a pipeline is not a team pipeline: it names a profile or tool that is not a team resource of the project.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TeamPipelineError {
  #[error("step `{step}` runs the agent profile {profile}, which is not a team agent profile of this project")]
  Profile { step: StepKey, profile: ResourceId },
  #[error("step `{step}` calls the tool {tool}, which is not a team tool of this project")]
  Tool { step: StepKey, tool: ResourceId },
  #[error(
    "the Deliver step resolves conflicts with the agent profile {profile}, which is not a team agent profile of this project"
  )]
  Integrator { profile: ResourceId },
}

impl Pipeline {
  /// Whether this pipeline can be the team's: it fails with the first profile or tool `team` does not have. A step
  /// whose profile or tool is not chosen yet names nothing; publishing refuses it separately
  /// ([`Pipeline::unassigned_steps`]). A team pipeline is sent unchanged, so there is no other form of it.
  pub fn check_team(&self, team: &TeamExecutors) -> Result<(), TeamPipelineError> {
    for step in self.steps() {
      match &step.kind {
        StepKind::Agent {
          profile: Some(profile), ..
        } if !team.profiles.contains(profile) => {
          return Err(TeamPipelineError::Profile {
            step: step.key.clone(),
            profile: *profile,
          });
        }
        StepKind::Tool { tool: Some(tool) } if !team.tools.contains(tool) => {
          return Err(TeamPipelineError::Tool {
            step: step.key.clone(),
            tool: *tool,
          });
        }
        StepKind::Agent { .. } | StepKind::Tool { .. } => {}
      }
    }
    match self.deliver().and_then(|deliver| deliver.integrator()) {
      Some(profile) if !team.profiles.contains(&profile) => Err(TeamPipelineError::Integrator { profile }),
      _ => Ok(()),
    }
  }
}
