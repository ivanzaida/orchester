//! Versioned configuration: agent profiles, instructions, pipelines, tools and skills share one
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
mod skill;
mod tool;
mod variable;

use serde::{Serialize, de::DeserializeOwned};

pub use self::{
  agent_profile::{AgentProfile, ModelId, ModelIdError, ProviderSettings, ProviderSettingsError},
  instruction::{
    AgentTargets, Exclusion, InjectionEvent, Instruction, InstructionError, InstructionScope, InstructionTarget,
  },
  lifecycle::{
    Draft, DraftSaved, PublishError, Resource, ResourceKind, Slug, SlugError, Version, VersionNumber, publish,
  },
  pipeline::{
    EditProblem, FailureRoute, Pipeline, PipelineChange, PipelineEdit, PipelineError, PipelineSources,
    PipelineSourcesError, RepositoryAccess, StarterAssignments, StarterStep, StarterStepText, Step, StepChange,
    StepKey, StepKind, Weaker, changes, feature_delivery, weaker_than,
  },
  prompt::{PromptError, PromptText, Rendered},
  skill::{SKILL_FILE, Skill, SkillError, SkillFile, SkillFilePath},
  tool::{
    AccessPolicy, AccessPolicyError, AccessWindow, AllowedSteps, ApprovalMode, CommandAction, CommandArg,
    CommandEnvironment, CommandError, CommandProgram, CommandTimeout, DatabaseQuery, DeniedBehavior, ExitCodes,
    FieldName, FieldNameError, HttpAuth, HttpError, HttpMethod, HttpRequest, OutputLimit, ProgramLocation,
    RequestTarget, ResponseSource, Tool, ToolAction, ToolError, ToolKind, VariableName,
  },
  variable::{UnknownVariable, Variable, VariableGroup, VariableType},
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
  /// 3: MCP servers (2026-09-28). Profiles of schemas 1 and 2 get none.
  /// 4: extra writable folders in the permissions (2026-09-28), left out while there are none, so
  /// profiles of schema 3 have the same shape.
  const SCHEMA: u32 = 4;

  fn upgrade(schema: u32, mut body: serde_json::Value) -> Option<serde_json::Value> {
    if schema == 3 {
      return Some(body);
    }
    let fields = body.as_object_mut()?;
    match schema {
      1 => {
        let permissions =
          serde_json::to_value(AgentPermissions::before_permissions()).expect("permissions serialize to JSON");
        fields.insert("permissions".to_owned(), permissions);
      }
      2 => {}
      _ => return None,
    }
    fields.insert("mcp_servers".to_owned(), serde_json::Value::Array(Vec::new()));
    Some(body)
  }
}

impl ResourceBody for Instruction {
  const KIND: ResourceKind = ResourceKind::Instruction;
  /// 2: targets may name agent profiles (2026-09-27). Bodies of schema 1 have the same shape.
  const SCHEMA: u32 = 2;

  fn upgrade(schema: u32, body: serde_json::Value) -> Option<serde_json::Value> {
    (schema == 1).then_some(body)
  }

  fn check_publishable(&self) -> Result<(), PublishError> {
    if self.text().is_blank() {
      Err(PublishError::BlankInstruction)
    } else {
      Ok(())
    }
  }
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
  /// 2: description, optional access policy and the command tool's timeout, expected exit codes,
  /// environment and output limit (M10a, 2026-09-27).
  const SCHEMA: u32 = 2;

  fn upgrade(schema: u32, body: serde_json::Value) -> Option<serde_json::Value> {
    (schema == 1).then(|| tool::upgrade_from_schema_1(body)).flatten()
  }
}

impl ResourceBody for Skill {
  const KIND: ResourceKind = ResourceKind::Skill;
  const SCHEMA: u32 = 1;

  fn check_publishable(&self) -> Result<(), PublishError> {
    if self.description().trim().is_empty() {
      Err(PublishError::BlankSkillDescription)
    } else if self.body().trim().is_empty() {
      Err(PublishError::BlankSkillBody)
    } else {
      Ok(())
    }
  }
}
