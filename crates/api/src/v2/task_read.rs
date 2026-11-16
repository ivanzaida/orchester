//! A task as an S10 read gives it: every public field of a shared task ([`SharedTask`]), then its assignee, its work,
//! its effective claim and the caller's capabilities on it. The workflow status stays apart from the work's execution
//! state. Every field is sent; one without a value is `null`, and an unknown field is refused. `closed` and `blocked`
//! came later: a server leaves them out when it has no record, and so does an older one.

use orchester_core::{
  id::{ModuleId, ReleaseId, TagId, TaskId},
  sharing::TaskRevision,
  task::{
    TaskStatus,
    organization::{Priority, WorkflowStatus},
  },
};
use serde::{Deserialize, Serialize};

use super::{Capabilities, ClaimRead, TaskClosure, WorkRead, page::nullable};
use crate::{Attribution, Blocking, RunStatus, SharedClaim, SharedLimits, SharedTask, TaskContent, UserRef, wire};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "TaskReadWire", into = "TaskReadWire")]
pub struct TaskRead {
  pub task: SharedTask,
  pub assignee: Option<UserRef>,
  pub work: Option<WorkRead>,
  /// The claim work on the task runs under: its own, or its module's.
  pub effective_claim: Option<ClaimRead>,
  pub capabilities: Capabilities,
  /// How and by whom the task was closed; only while it is Done or Canceled. `None` also when the server does not say
  /// (an older server leaves the field out): the task's closing is then unknown.
  pub closed: Option<TaskClosure>,
  /// Who set the task's current blocked reason, and when; only while it has one. `None` also when the server does not
  /// say.
  pub blocked: Option<Attribution>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskReadWire {
  id: TaskId,
  number: u32,
  key: String,
  #[serde(deserialize_with = "nullable")]
  former_key: Option<String>,
  revision: TaskRevision,
  title: String,
  brief: String,
  criteria: Vec<String>,
  #[serde(deserialize_with = "nullable")]
  pipeline: Option<String>,
  #[serde(deserialize_with = "nullable")]
  limits: Option<SharedLimits>,
  #[serde(with = "wire::Workflow")]
  workflow: WorkflowStatus,
  #[serde(with = "wire::PriorityWire")]
  priority: Priority,
  tags: Vec<TagId>,
  blocking: Vec<Blocking>,
  #[serde(deserialize_with = "nullable")]
  blocked_reason: Option<String>,
  #[serde(deserialize_with = "nullable")]
  module: Option<ModuleId>,
  #[serde(deserialize_with = "nullable")]
  release: Option<ReleaseId>,
  #[serde(with = "wire::Status")]
  status: TaskStatus,
  has_run: bool,
  created: Attribution,
  updated: Attribution,
  #[serde(deserialize_with = "nullable")]
  claim: Option<SharedClaim>,
  #[serde(deserialize_with = "nullable")]
  run: Option<RunStatus>,
  #[serde(deserialize_with = "nullable")]
  assignee: Option<UserRef>,
  #[serde(deserialize_with = "nullable")]
  work: Option<WorkRead>,
  #[serde(deserialize_with = "nullable")]
  effective_claim: Option<ClaimRead>,
  capabilities: Capabilities,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  closed: Option<TaskClosure>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  blocked: Option<Attribution>,
}

impl From<TaskReadWire> for TaskRead {
  fn from(wire: TaskReadWire) -> Self {
    let content = TaskContent {
      title: wire.title,
      brief: wire.brief,
      criteria: wire.criteria,
      pipeline: wire.pipeline,
      limits: wire.limits,
      workflow: wire.workflow,
      priority: wire.priority,
      tags: wire.tags,
      blocking: wire.blocking,
      blocked_reason: wire.blocked_reason,
      module: wire.module,
      release: wire.release,
    };
    Self {
      task: SharedTask {
        id: wire.id,
        number: wire.number,
        key: wire.key,
        former_key: wire.former_key,
        revision: wire.revision,
        content,
        status: wire.status,
        has_run: wire.has_run,
        created: wire.created,
        updated: wire.updated,
        claim: wire.claim,
        run: wire.run,
      },
      assignee: wire.assignee,
      work: wire.work,
      effective_claim: wire.effective_claim,
      capabilities: wire.capabilities,
      closed: wire.closed,
      blocked: wire.blocked,
    }
  }
}

impl From<TaskRead> for TaskReadWire {
  fn from(read: TaskRead) -> Self {
    let SharedTask {
      id,
      number,
      key,
      former_key,
      revision,
      content,
      status,
      has_run,
      created,
      updated,
      claim,
      run,
    } = read.task;
    Self {
      id,
      number,
      key,
      former_key,
      revision,
      title: content.title,
      brief: content.brief,
      criteria: content.criteria,
      pipeline: content.pipeline,
      limits: content.limits,
      workflow: content.workflow,
      priority: content.priority,
      tags: content.tags,
      blocking: content.blocking,
      blocked_reason: content.blocked_reason,
      module: content.module,
      release: content.release,
      status,
      has_run,
      created,
      updated,
      claim,
      run,
      assignee: read.assignee,
      work: read.work,
      effective_claim: read.effective_claim,
      capabilities: read.capabilities,
      closed: read.closed,
      blocked: read.blocked,
    }
  }
}
