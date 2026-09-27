use std::str::FromStr;

use thiserror::Error;

/// A value that prompt text can reference as `{{name}}`. This is the proposed catalog of the
/// design ("Variable catalog proposal"); secret values are deliberately not part of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Variable {
  TaskId,
  TaskTitle,
  TaskDescription,
  TaskAcceptanceCriteria,
  TaskAttachments,
  TaskCreatedAt,
  ProjectId,
  ProjectName,
  ProjectRepository,
  ProjectDefaultBranch,
  AgentId,
  AgentName,
  AgentRole,
  AgentModel,
  PipelineId,
  PipelineName,
  PipelineVersion,
  RunId,
  RunAttempt,
  RunBudget,
  RunBranch,
  RunRevision,
  StepId,
  StepName,
}

impl Variable {
  pub const ALL: [Self; 24] = [
    Self::TaskId,
    Self::TaskTitle,
    Self::TaskDescription,
    Self::TaskAcceptanceCriteria,
    Self::TaskAttachments,
    Self::TaskCreatedAt,
    Self::ProjectId,
    Self::ProjectName,
    Self::ProjectRepository,
    Self::ProjectDefaultBranch,
    Self::AgentId,
    Self::AgentName,
    Self::AgentRole,
    Self::AgentModel,
    Self::PipelineId,
    Self::PipelineName,
    Self::PipelineVersion,
    Self::RunId,
    Self::RunAttempt,
    Self::RunBudget,
    Self::RunBranch,
    Self::RunRevision,
    Self::StepId,
    Self::StepName,
  ];

  /// The name written between the braces, e.g. `task.title`.
  pub const fn name(self) -> &'static str {
    match self {
      Self::TaskId => "task.id",
      Self::TaskTitle => "task.title",
      Self::TaskDescription => "task.description",
      Self::TaskAcceptanceCriteria => "task.acceptance_criteria",
      Self::TaskAttachments => "task.attachments",
      Self::TaskCreatedAt => "task.created_at",
      Self::ProjectId => "project.id",
      Self::ProjectName => "project.name",
      Self::ProjectRepository => "project.repository",
      Self::ProjectDefaultBranch => "project.default_branch",
      Self::AgentId => "agent.id",
      Self::AgentName => "agent.name",
      Self::AgentRole => "agent.role",
      Self::AgentModel => "agent.model",
      Self::PipelineId => "pipeline.id",
      Self::PipelineName => "pipeline.name",
      Self::PipelineVersion => "pipeline.version",
      Self::RunId => "run.id",
      Self::RunAttempt => "run.attempt",
      Self::RunBudget => "run.budget",
      Self::RunBranch => "run.branch",
      Self::RunRevision => "run.revision",
      Self::StepId => "step.id",
      Self::StepName => "step.name",
    }
  }
  /// The part of the catalog the variable belongs to.
  pub const fn group(self) -> VariableGroup {
    match self {
      Self::TaskId
      | Self::TaskTitle
      | Self::TaskDescription
      | Self::TaskAcceptanceCriteria
      | Self::TaskAttachments
      | Self::TaskCreatedAt => VariableGroup::Task,
      Self::ProjectId | Self::ProjectName | Self::ProjectRepository | Self::ProjectDefaultBranch => {
        VariableGroup::Project
      }
      Self::AgentId | Self::AgentName | Self::AgentRole | Self::AgentModel => VariableGroup::Agent,
      Self::PipelineId | Self::PipelineName | Self::PipelineVersion => VariableGroup::Pipeline,
      Self::RunId | Self::RunAttempt | Self::RunBudget | Self::RunBranch | Self::RunRevision => VariableGroup::Run,
      Self::StepId | Self::StepName => VariableGroup::Step,
    }
  }

  /// What kind of value the variable holds (the design's "Type" column).
  pub const fn value_type(self) -> VariableType {
    match self {
      Self::TaskAcceptanceCriteria => VariableType::StringList,
      Self::TaskAttachments => VariableType::AttachmentList,
      Self::TaskCreatedAt => VariableType::Timestamp,
      Self::ProjectRepository => VariableType::Repository,
      Self::PipelineVersion | Self::RunAttempt => VariableType::Integer,
      Self::RunBudget => VariableType::Tokens,
      _ => VariableType::String,
    }
  }
}

/// The catalog's groups, in the design's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VariableGroup {
  Task,
  Project,
  Agent,
  Pipeline,
  Run,
  Step,
}

impl VariableGroup {
  pub const ALL: [Self; 6] = [
    Self::Task,
    Self::Project,
    Self::Agent,
    Self::Pipeline,
    Self::Run,
    Self::Step,
  ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VariableType {
  String,
  StringList,
  AttachmentList,
  Timestamp,
  Repository,
  Integer,
  Tokens,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not a variable in the catalog")]
pub struct UnknownVariable(pub String);

impl FromStr for Variable {
  type Err = UnknownVariable;

  fn from_str(name: &str) -> Result<Self, UnknownVariable> {
    Self::ALL
      .into_iter()
      .find(|variable| variable.name() == name)
      .ok_or_else(|| UnknownVariable(name.to_owned()))
  }
}
