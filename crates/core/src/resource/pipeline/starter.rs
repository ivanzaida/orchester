//! The "Feature delivery" starter pipeline of the design ("Core workflow"): Research → Plan
//! (plan approval) → Implement → Run checks ∥ Review → Create pull request. Failed checks and
//! requested changes return to Implement, up to [`STARTER_RETRIES`] times.
//!
//! Step names and instructions are user-facing text, so the caller supplies them (translated).
//! Profiles and tools are chosen by the caller too; any left unset makes a draft that says what
//! is missing and cannot be published until it is chosen.

use std::{collections::BTreeSet, num::NonZeroU32};

use super::{FailureRoute, Pipeline, RepositoryAccess, Step, StepKey, StepKind};
use crate::{id::ResourceId, name::Name, resource::prompt::PromptText};

/// How often failed checks or requested changes may return to Implement.
const STARTER_RETRIES: NonZeroU32 = NonZeroU32::new(2).expect("two is not zero");

/// A step of the starter pipeline, in the order it is listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StarterStep {
  Research,
  Plan,
  Implement,
  RunChecks,
  Review,
  CreatePullRequest,
}

impl StarterStep {
  pub const ALL: [Self; 6] = [
    Self::Research,
    Self::Plan,
    Self::Implement,
    Self::RunChecks,
    Self::Review,
    Self::CreatePullRequest,
  ];

  /// The stable step key.
  pub const fn key(self) -> &'static str {
    match self {
      Self::Research => "research",
      Self::Plan => "plan",
      Self::Implement => "implement",
      Self::RunChecks => "run-checks",
      Self::Review => "review",
      Self::CreatePullRequest => "create-pull-request",
    }
  }

  /// Run checks and Create pull request call tools; the other steps run agents.
  pub const fn is_tool(self) -> bool {
    matches!(self, Self::RunChecks | Self::CreatePullRequest)
  }

  const fn depends_on(self) -> &'static [Self] {
    match self {
      Self::Research => &[],
      Self::Plan => &[Self::Research],
      Self::Implement => &[Self::Plan],
      Self::RunChecks | Self::Review => &[Self::Implement],
      Self::CreatePullRequest => &[Self::RunChecks, Self::Review],
    }
  }

  const fn repository(self) -> RepositoryAccess {
    match self {
      Self::Implement => RepositoryAccess::Write,
      _ => RepositoryAccess::ReadOnly,
    }
  }

  fn step_key(self) -> StepKey {
    StepKey::new(self.key()).expect("starter step keys are valid slugs")
  }
}

/// Name and instructions of one starter step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StarterStepText {
  pub name: Name,
  pub instructions: PromptText,
}

/// Published project resources to run the starter steps with. `None` leaves the step unset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StarterAssignments {
  /// Agent profile of Research (the design's Explorer).
  pub explorer: Option<ResourceId>,
  /// Agent profile of Plan (Planner).
  pub planner: Option<ResourceId>,
  /// Agent profile of Implement (Builder).
  pub builder: Option<ResourceId>,
  /// Agent profile of Review (Reviewer).
  pub reviewer: Option<ResourceId>,
  /// Tool of Run checks.
  pub checks: Option<ResourceId>,
  /// Tool of Create pull request.
  pub pull_request: Option<ResourceId>,
}

impl StarterAssignments {
  const fn executor(&self, step: StarterStep) -> Option<ResourceId> {
    match step {
      StarterStep::Research => self.explorer,
      StarterStep::Plan => self.planner,
      StarterStep::Implement => self.builder,
      StarterStep::Review => self.reviewer,
      StarterStep::RunChecks => self.checks,
      StarterStep::CreatePullRequest => self.pull_request,
    }
  }
}

/// Builds the starter pipeline with the names and instructions `text` gives each step.
pub fn feature_delivery(text: impl Fn(StarterStep) -> StarterStepText, assignments: StarterAssignments) -> Pipeline {
  let steps = StarterStep::ALL
    .into_iter()
    .map(|starter| {
      let StarterStepText { name, instructions } = text(starter);
      let executor = assignments.executor(starter);
      Step {
        key: starter.step_key(),
        name,
        kind: if starter.is_tool() {
          StepKind::Tool { tool: executor }
        } else {
          StepKind::Agent {
            profile: executor,
            repository: starter.repository(),
            model: None,
          }
        },
        instructions,
        depends_on: starter
          .depends_on()
          .iter()
          .map(|dependency| dependency.step_key())
          .collect::<BTreeSet<_>>(),
        approval_gate: starter == StarterStep::Plan,
        on_failure: matches!(starter, StarterStep::RunChecks | StarterStep::Review).then(|| FailureRoute {
          return_to: StarterStep::Implement.step_key(),
          max_retries: STARTER_RETRIES,
        }),
        budget: None,
      }
    })
    .collect();
  Pipeline::new(steps).expect("the starter pipeline satisfies the pipeline rules")
}
