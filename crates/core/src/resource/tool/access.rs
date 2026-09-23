use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{id::ResourceId, name::Name, resource::StepKey};

/// Steps of an allowed pipeline that may call the tool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllowedSteps {
  All,
  Only(BTreeSet<StepKey>),
}

/// When a person has to approve a call before the credential is released.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
  EveryCall,
  OncePerRun,
  Never,
}

/// How long granted access lasts. Access never outlives the step that requested it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessWindow {
  UntilStepEnds,
  Minutes(NonZeroU32),
}

/// What happens to a step whose call is denied. Denial never erases the step's work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeniedBehavior {
  /// The step waits for a changed plan or a new decision.
  AwaitChangedPlan,
  /// The step fails and follows its failure route.
  FailStep,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AccessPolicyError {
  #[error("pipeline {0} is allowed for selected steps but no step is selected")]
  NoStepsSelected(ResourceId),
}

/// Who may call a tool and under which conditions. Callers not listed are denied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AccessPolicyFields")]
pub struct AccessPolicy {
  callers: BTreeMap<ResourceId, AllowedSteps>,
  environment: Name,
  approval: ApprovalMode,
  window: AccessWindow,
  denied: DeniedBehavior,
}

#[derive(Deserialize)]
struct AccessPolicyFields {
  callers: BTreeMap<ResourceId, AllowedSteps>,
  environment: Name,
  approval: ApprovalMode,
  window: AccessWindow,
  denied: DeniedBehavior,
}

impl TryFrom<AccessPolicyFields> for AccessPolicy {
  type Error = AccessPolicyError;

  fn try_from(fields: AccessPolicyFields) -> Result<Self, AccessPolicyError> {
    Self::new(
      fields.callers,
      fields.environment,
      fields.approval,
      fields.window,
      fields.denied,
    )
  }
}

impl AccessPolicy {
  /// `callers` maps each allowed pipeline to its allowed steps.
  pub fn new(
    callers: BTreeMap<ResourceId, AllowedSteps>,
    environment: Name,
    approval: ApprovalMode,
    window: AccessWindow,
    denied: DeniedBehavior,
  ) -> Result<Self, AccessPolicyError> {
    let empty = callers
      .iter()
      .find(|(_, steps)| matches!(steps, AllowedSteps::Only(keys) if keys.is_empty()));
    if let Some((&pipeline, _)) = empty {
      return Err(AccessPolicyError::NoStepsSelected(pipeline));
    }
    Ok(Self {
      callers,
      environment,
      approval,
      window,
      denied,
    })
  }

  pub fn callers(&self) -> &BTreeMap<ResourceId, AllowedSteps> {
    &self.callers
  }

  /// Environment whose secrets the tool uses, e.g. `Staging`.
  pub fn environment(&self) -> &Name {
    &self.environment
  }

  pub const fn approval(&self) -> ApprovalMode {
    self.approval
  }

  pub const fn window(&self) -> AccessWindow {
    self.window
  }

  pub const fn denied(&self) -> DeniedBehavior {
    self.denied
  }

  /// Whether `step` of `pipeline` is on the allowlist.
  pub fn allows(&self, pipeline: ResourceId, step: &StepKey) -> bool {
    match self.callers.get(&pipeline) {
      Some(AllowedSteps::All) => true,
      Some(AllowedSteps::Only(steps)) => steps.contains(step),
      None => false,
    }
  }
}
