//! A pipeline as the team server holds it (`docs/design/team-configuration.md`, "Per resource kind"): each step runs an
//! agent profile or calls a tool of the team, and the Deliver step's integrator is a profile of the team, every one
//! named by its id on the server. A team resource keeps the server's id on every computer, so the body needs no
//! rewriting; what the team body must rule out is an id of a resource the server does not have, such as a profile only
//! this computer keeps. A local pipeline keeps naming its own profiles and tools and never becomes a [`TeamPipeline`].
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

/// A pipeline whose steps and Deliver step name only team resources. It is the body the team receives, unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamPipeline(Pipeline);

impl TeamPipeline {
  /// `pipeline` as a team body, which fails with the first profile or tool `team` does not have. A step whose profile
  /// or tool is not chosen yet names nothing; publishing refuses it separately ([`Pipeline::unassigned_steps`]).
  pub fn new(pipeline: Pipeline, team: &TeamExecutors) -> Result<Self, TeamPipelineError> {
    for step in pipeline.steps() {
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
    if let Some(profile) = pipeline.deliver().and_then(|deliver| deliver.integrator())
      && !team.profiles.contains(&profile)
    {
      return Err(TeamPipelineError::Integrator { profile });
    }
    Ok(Self(pipeline))
  }

  pub const fn pipeline(&self) -> &Pipeline {
    &self.0
  }

  pub fn into_pipeline(self) -> Pipeline {
    self.0
  }
}
