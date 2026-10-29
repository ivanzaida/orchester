//! Versioned configuration: agent profiles, instructions, pipelines, tools, skills, test suites,
//! services and stacks share one lifecycle — one editable draft and immutable published versions
//! `v1..vN`. Runs pin versions, so publishing affects new runs only.
//!
//! Bodies reference other resources, connections and secrets by id only, except a test suite, a
//! service and a stack, which name their secrets and the services they need ([`TestSuite`],
//! [`crate::service`]). Which published version of a referenced resource is used is decided when a
//! run snapshots its configuration, not inside a body.

mod agent_profile;
mod instruction;
mod lifecycle;
mod pipeline;
mod prompt;
mod removal;
mod skill;
mod test_suite;
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
    DeliverChange, DeliverMode, DeliverStep, Deliverable, EditProblem, FailureRoute, MergeMethod, Pipeline,
    PipelineChange, PipelineEdit, PipelineError, PipelineSources, PipelineSourcesError, RepositoryAccess,
    StarterAssignments, StarterStep, StarterStepText, Step, StepChange, StepKey, StepKind, UnknownDeliverable, Weaker,
    WithoutCommits, changes, feature_delivery, takes_base_moves, takes_base_moves_by_default, weaker_than,
  },
  prompt::{PromptError, PromptReference, PromptText, Rendered, prompt_references},
  removal::{Removal, RemoveMode, StepUser, StepVersion, pipeline_step_users, step_users},
  skill::{SKILL_FILE, Skill, SkillError, SkillFile, SkillFilePath},
  test_suite::{
    ArtifactGlob, ReportFormat, ReportOutput, SuiteCommand, SuiteNeeds, SuitePath, SuiteVariable, SuiteVariables,
    TestFilter, TestLayer, TestReport, TestSuite, TestSuiteError,
  },
  tool::{
    AccessPolicy, AccessPolicyError, AccessWindow, AllowedSteps, ApprovalMode, CommandAction, CommandArg,
    CommandEnvironment, CommandError, CommandProgram, CommandTimeout, DatabaseQuery, DeniedBehavior, ExitCodes,
    FieldName, FieldNameError, HttpAuth, HttpError, HttpMethod, HttpRequest, NO_CURRENT_DIRECTORY_IN_EXE_PATH,
    OutputLimit, ProgramLocation, RequestTarget, ResponseSource, Tool, ToolAction, ToolError, ToolKind, VariableName,
  },
  variable::{UnknownVariable, Variable, VariableGroup, VariableType},
};
use crate::{
  permission::AgentPermissions,
  service::{PublishNeeds, Service, Stack},
};

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

  /// The needs a service or stack must find among the project's published services to be published; `None`
  /// for every other kind.
  fn publish_needs(&self) -> Option<PublishNeeds<'_>> {
    None
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

impl ResourceBody for TestSuite {
  const KIND: ResourceKind = ResourceKind::TestSuite;
  const SCHEMA: u32 = 1;
}

impl ResourceBody for Service {
  const KIND: ResourceKind = ResourceKind::Service;
  const SCHEMA: u32 = 1;

  fn publish_needs(&self) -> Option<PublishNeeds<'_>> {
    Some(PublishNeeds::Service(self))
  }
}

impl ResourceBody for Stack {
  const KIND: ResourceKind = ResourceKind::Stack;
  const SCHEMA: u32 = 1;

  fn publish_needs(&self) -> Option<PublishNeeds<'_>> {
    Some(PublishNeeds::Stack(self))
  }
}
