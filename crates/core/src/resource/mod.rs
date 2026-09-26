//! Versioned configuration: agent profiles, instructions, pipelines and tools share one
//! lifecycle — one editable draft and immutable published versions `v1..vN`. Runs pin
//! versions, so publishing affects new runs only.
//!
//! Bodies reference other resources, connections and secrets by id only. Which published
//! version of a referenced resource is used is decided when a run snapshots its configuration,
//! not inside a body.

mod agent_profile;
mod instruction;
mod lifecycle;
mod pipeline;
mod prompt;
mod tool;
mod variable;

use serde::{Serialize, de::DeserializeOwned};

pub use self::{
  agent_profile::{AgentProfile, ModelId, ModelIdError, ProviderSettings, ProviderSettingsError},
  instruction::{AgentTargets, InjectionEvent, Instruction, InstructionError, InstructionScope},
  lifecycle::{
    Draft, DraftSaved, PublishError, Resource, ResourceKind, Slug, SlugError, Version, VersionNumber, publish,
  },
  pipeline::{
    EditProblem, FailureRoute, Pipeline, PipelineChange, PipelineEdit, PipelineError, RepositoryAccess,
    StarterAssignments, StarterStep, StarterStepText, Step, StepChange, StepKey, StepKind, changes, feature_delivery,
  },
  prompt::{PromptError, PromptText},
  tool::{
    AccessPolicy, AccessPolicyError, AccessWindow, AllowedSteps, ApprovalMode, CommandAction, DatabaseQuery,
    DeniedBehavior, FieldName, FieldNameError, HttpAuth, HttpError, HttpMethod, HttpRequest, RequestTarget,
    ResponseSource, Tool, ToolAction, ToolError,
  },
  variable::{UnknownVariable, Variable},
};
use crate::permission::AgentPermissions;

/// Content of a versioned resource kind, stored as JSON. Deserializing re-runs the domain
/// constructors, so a stored body that breaks a rule is rejected instead of loaded.
pub trait ResourceBody: Serialize + DeserializeOwned + PartialEq {
  const KIND: ResourceKind;
  /// Version of the stored JSON shape. Changing the shape requires a new number and a
  /// conversion from every older one ([`Self::upgrade`]).
  const SCHEMA: u32;

  /// A body stored with an older `schema`, converted to the current shape; `None` for a schema
  /// this kind never had.
  fn upgrade(_schema: u32, _body: serde_json::Value) -> Option<serde_json::Value> {
    None
  }

  /// Rules a body must meet to be published beyond those of a draft. Every
  /// [`crate::store::ResourceStore::publish_draft`] implementation checks it before publishing.
  fn check_publishable(&self) -> Result<(), PublishError> {
    Ok(())
  }
}

impl ResourceBody for AgentProfile {
  const KIND: ResourceKind = ResourceKind::AgentProfile;
  /// 2: permissions (2026-09-26). Profiles of schema 1 get
  /// [`AgentPermissions::before_permissions`].
  const SCHEMA: u32 = 2;

  fn upgrade(schema: u32, mut body: serde_json::Value) -> Option<serde_json::Value> {
    if schema != 1 {
      return None;
    }
    let permissions =
      serde_json::to_value(AgentPermissions::before_permissions()).expect("permissions serialize to JSON");
    body.as_object_mut()?.insert("permissions".to_owned(), permissions);
    Some(body)
  }
}

impl ResourceBody for Instruction {
  const KIND: ResourceKind = ResourceKind::Instruction;
  const SCHEMA: u32 = 1;
}

impl ResourceBody for Pipeline {
  const KIND: ResourceKind = ResourceKind::Pipeline;
  const SCHEMA: u32 = 1;

  fn check_publishable(&self) -> Result<(), PublishError> {
    if self.steps().is_empty() {
      return Err(PublishError::NoSteps);
    }
    let unassigned: Vec<StepKey> = self.unassigned_steps().cloned().collect();
    if unassigned.is_empty() {
      Ok(())
    } else {
      Err(PublishError::UnassignedSteps(unassigned))
    }
  }
}

impl ResourceBody for Tool {
  const KIND: ResourceKind = ResourceKind::Tool;
  const SCHEMA: u32 = 1;
}
