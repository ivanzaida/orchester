//! The canonical part of a run's inputs: what was accepted for the run, by the team server for a team run or by this
//! computer for a local one, apart from how this computer binds it.

use std::{num::NonZeroU32, time::Duration};

use serde::{Deserialize, Serialize};

use super::{
  InputsError,
  labels::{self, ResourceLabel},
};
use crate::{
  blob::ContentHash,
  id::{ClaimId, ModuleId, OperationId, ResourceId, WorkId},
  name::Name,
  resource::Deliverable,
  run::{
    BranchName, CommitId,
    bundle::{CanonicalError, CanonicalExecutionInputs, bundle_sha256},
  },
  sharing::TaskRevision,
  task::{TaskLimits, identity::TeamTaskRef},
  tokens::TokenBudget,
};

/// The version of the stored inputs' shape; 2 added the task's number and the pinned resources' labels.
pub const RUN_INPUTS_FORMAT: u32 = 2;

/// `{title, brief, criteria}`: the task the run executes, as it was when the run started.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskAtStart {
  pub title: String,
  pub brief: String,
  pub criteria: Vec<String>,
}

/// `{task, definition_revision, work, claim, operation, module, branch, base_commit, bundle}`: what the team server
/// accepted when it started the work the run carries out (S7 `work.start`, S8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeamAcceptance {
  pub task: TeamTaskRef,
  /// The task's revision when the work started.
  pub definition_revision: TaskRevision,
  pub work: WorkId,
  /// The claim the work was started under; each acquisition has a new one, which fences the ones before.
  pub claim: ClaimId,
  /// The accepted operation that started the work.
  pub operation: OperationId,
  pub module: ModuleId,
  /// The module's branch the run's work merges into.
  pub branch: BranchName,
  /// The commit of the base branch the work starts from.
  pub base_commit: CommitId,
  /// The hash of the bundle the server built (S8 `BundleRef`).
  pub bundle: ContentHash,
}

/// `{kind: "local"}` or `{kind: "team", ...}`: who accepted the inputs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputsOrigin {
  /// A local project's run, of this computer's published versions.
  Local,
  Team(TeamAcceptance),
}

/// `{format, origin, task_number, task, labels, budget_tokens?, max_duration_ms?, delivery, inputs}`: everything the
/// run executes with that does not depend on this computer. Nothing in it is a secret value, an account, a connection
/// or a path of one computer ([`CanonicalExecutionInputs`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BundleFields")]
pub struct BundleContent {
  format: u32,
  origin: InputsOrigin,
  /// The task's number in its project, its key's (`ORC-129`).
  task_number: NonZeroU32,
  task: TaskAtStart,
  /// One per resource the inputs pin, in resource id order.
  labels: Vec<ResourceLabel>,
  #[serde(skip_serializing_if = "Option::is_none")]
  budget_tokens: Option<TokenBudget>,
  #[serde(skip_serializing_if = "Option::is_none")]
  max_duration_ms: Option<u64>,
  delivery: Deliverable,
  inputs: CanonicalExecutionInputs,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleFields {
  format: u32,
  origin: InputsOrigin,
  task_number: NonZeroU32,
  task: TaskAtStart,
  labels: Vec<ResourceLabel>,
  #[serde(default)]
  budget_tokens: Option<TokenBudget>,
  #[serde(default)]
  max_duration_ms: Option<u64>,
  delivery: Deliverable,
  inputs: CanonicalExecutionInputs,
}

impl TryFrom<BundleFields> for BundleContent {
  type Error = InputsError;

  fn try_from(fields: BundleFields) -> Result<Self, InputsError> {
    if fields.format != RUN_INPUTS_FORMAT {
      return Err(InputsError::Format(fields.format));
    }
    TaskLimits::new(fields.budget_tokens, fields.max_duration_ms.map(Duration::from_millis))?;
    Name::new(&fields.task.title).map_err(InputsError::TaskTitle)?;
    Ok(Self {
      format: fields.format,
      origin: fields.origin,
      task_number: fields.task_number,
      task: fields.task,
      labels: labels::checked(fields.labels, &fields.inputs.resources())?,
      budget_tokens: fields.budget_tokens,
      max_duration_ms: fields.max_duration_ms,
      delivery: fields.delivery,
      inputs: fields.inputs,
    })
  }
}

impl BundleContent {
  /// The content of `inputs` accepted by `origin` for the task numbered `number`, as it was then, with a label for
  /// each resource the inputs pin.
  pub fn new(
    origin: InputsOrigin,
    (number, task): (NonZeroU32, TaskAtStart),
    labels: Vec<ResourceLabel>,
    (limits, delivery): (TaskLimits, Deliverable),
    inputs: CanonicalExecutionInputs,
  ) -> Result<Self, InputsError> {
    let max_duration_ms = limits
      .max_duration()
      .map(|limit| u64::try_from(limit.as_millis()))
      .transpose()
      .map_err(|_| InputsError::Unrepresentable("time limit"))?;
    Name::new(&task.title).map_err(InputsError::TaskTitle)?;
    Ok(Self {
      format: RUN_INPUTS_FORMAT,
      origin,
      task_number: number,
      task,
      labels: labels::checked(labels, &inputs.resources())?,
      budget_tokens: limits.budget(),
      max_duration_ms,
      delivery,
      inputs,
    })
  }

  pub const fn origin(&self) -> &InputsOrigin {
    &self.origin
  }

  pub const fn task_number(&self) -> NonZeroU32 {
    self.task_number
  }

  /// The task as it was when the run started; its title is a [`Name`].
  pub const fn task(&self) -> &TaskAtStart {
    &self.task
  }

  /// The task's title, which was checked when the content was made or read.
  pub fn task_title(&self) -> Name {
    Name::new(&self.task.title).expect("the title was checked when the content was made or read")
  }

  /// The label of pinned resource `resource`; `None` for a resource the inputs do not pin.
  pub fn label(&self, resource: ResourceId) -> Option<&ResourceLabel> {
    self.labels.iter().find(|label| label.resource == resource)
  }

  pub fn limits(&self) -> TaskLimits {
    TaskLimits::new(self.budget_tokens, self.max_duration_ms.map(Duration::from_millis))
      .expect("the limits were checked when the content was made or read")
  }

  pub const fn delivery(&self) -> Deliverable {
    self.delivery
  }

  pub const fn inputs(&self) -> &CanonicalExecutionInputs {
    &self.inputs
  }

  /// The hash of the content's canonical bytes ([`crate::run::bundle::canonical`]).
  pub fn sha256(&self) -> Result<ContentHash, CanonicalError> {
    bundle_sha256(self)
  }
}
