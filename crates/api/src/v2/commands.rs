//! The team commands the desktop sends on the S4 envelopes (`CommandRequest<TeamCommand>`, answered with
//! `CommandResult<TeamCommandValue>`), team authority P2 (`docs/cloud-api.md`, "Team authority P2 commands"): task,
//! module and tag edits, module branches, assignment, claims, the workspace policy and delivery authorization. Each is
//! internally tagged by its `kind`; a patch is a list of `{field, value}` entries, so leaving a field out and setting
//! it to `null` differ. Fields whose owner is still open (a task's pipeline, limits and workflow, D4/D6) have no patch
//! entry: nothing here can set them. Every value carries the local effects the server authorized.

use orchester_core::{
  id::{ClaimId, ModuleId, TagId, TaskId, UserId, WorkId},
  module::ModuleState,
  run::{BranchName, CommitId},
  sharing::TaskRevision,
  task::organization::Priority,
};
use serde::{Deserialize, Serialize};

use super::{
  AuthorityRef, BundleRef, ClaimRead, DeliveryRead, LocalEffect, ModuleRead, Subject, TaskRead, WorkspacePolicy,
  page::nullable,
};
use crate::{Blocking, ModuleFields, SharedTag, TaskContent, UserRef, wire};

/// One field of a task a `task.patch` sets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "field", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskField {
  Title(String),
  Brief(String),
  Criteria(Vec<String>),
  Priority(#[serde(with = "wire::PriorityWire")] Priority),
  Tags(Vec<TagId>),
  Blocking(Vec<Blocking>),
  BlockedReason(Option<String>),
  /// The module the task moves to; `null` takes it out of its module.
  Module(Option<ModuleId>),
}

/// One field of a module a `module.patch` sets; its days are `YYYY-MM-DD`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "field", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModuleField {
  Name(String),
  Description(String),
  Start(Option<String>),
  Target(Option<String>),
  State(#[serde(with = "wire::ModuleStateWire")] ModuleState),
}

/// A team command, as `CommandRequest::command` carries it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum TeamCommand {
  #[serde(rename = "task.create")]
  TaskCreate { content: TaskContent },
  #[serde(rename = "task.patch")]
  TaskPatch { task_id: TaskId, changes: Vec<TaskField> },
  #[serde(rename = "module.create")]
  ModuleCreate { content: ModuleFields },
  #[serde(rename = "module.patch")]
  ModulePatch {
    module_id: ModuleId,
    changes: Vec<ModuleField>,
  },
  /// The module's branch, assigned by an Admin; `null` gives the module its automatic branch again.
  #[serde(rename = "module.branch")]
  ModuleBranch {
    module_id: ModuleId,
    #[serde(deserialize_with = "nullable")]
    branch: Option<BranchName>,
  },
  #[serde(rename = "tag.create")]
  TagCreate { name: String },
  #[serde(rename = "tag.rename")]
  TagRename { tag_id: TagId, name: String },
  #[serde(rename = "tag.merge")]
  TagMerge { tag_id: TagId, into: TagId },
  #[serde(rename = "tag.delete")]
  TagDelete { tag_id: TagId },
  /// Assigns the subject to `user_id`, or to nobody; clearing it fences the subject's claim at once.
  #[serde(rename = "assignment.set")]
  AssignmentSet {
    subject: Subject,
    #[serde(deserialize_with = "nullable")]
    user_id: Option<UserId>,
  },
  #[serde(rename = "claim.acquire")]
  ClaimAcquire { subject: Subject },
  #[serde(rename = "claim.take_over")]
  ClaimTakeOver {
    subject: Subject,
    observed_claim_id: ClaimId,
  },
  #[serde(rename = "claim.release")]
  ClaimRelease { authority: AuthorityRef },
  /// The workspace's automatic merge, under the policy revision the expected `project` revision names.
  #[serde(rename = "policy.set")]
  PolicySet { automatic_merge: bool },
  #[serde(rename = "delivery.authorize")]
  DeliveryAuthorize {
    work_id: WorkId,
    claim_id: ClaimId,
    source_commit: CommitId,
    base_commit: CommitId,
    bundle: BundleRef,
  },
}

impl TeamCommand {
  /// The command's `kind` on the wire, e.g. `task.patch`.
  pub const fn kind(&self) -> &'static str {
    match self {
      Self::TaskCreate { .. } => "task.create",
      Self::TaskPatch { .. } => "task.patch",
      Self::ModuleCreate { .. } => "module.create",
      Self::ModulePatch { .. } => "module.patch",
      Self::ModuleBranch { .. } => "module.branch",
      Self::TagCreate { .. } => "tag.create",
      Self::TagRename { .. } => "tag.rename",
      Self::TagMerge { .. } => "tag.merge",
      Self::TagDelete { .. } => "tag.delete",
      Self::AssignmentSet { .. } => "assignment.set",
      Self::ClaimAcquire { .. } => "claim.acquire",
      Self::ClaimTakeOver { .. } => "claim.take_over",
      Self::ClaimRelease { .. } => "claim.release",
      Self::PolicySet { .. } => "policy.set",
      Self::DeliveryAuthorize { .. } => "delivery.authorize",
    }
  }
}

/// What a committed team command answers, as `CommandResult::result` carries it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TeamCommandValue {
  /// `task.create`, `task.patch`.
  Task {
    task: Box<TaskRead>,
    effects: Vec<LocalEffect>,
  },
  /// `module.create`, `module.patch`, `module.branch`.
  Module {
    module: Box<ModuleRead>,
    effects: Vec<LocalEffect>,
  },
  /// The tag commands, with the tasks whose tags they changed.
  Tags {
    tags: Vec<SharedTag>,
    changed_tasks: Vec<TaskId>,
    effects: Vec<LocalEffect>,
  },
  Assignment {
    subject: Subject,
    #[serde(deserialize_with = "nullable")]
    assignee: Option<UserRef>,
    revision: TaskRevision,
    effects: Vec<LocalEffect>,
  },
  /// `claim.acquire`, `claim.take_over`: the claim, and a module's the claims of its tasks.
  Claim {
    subject: Subject,
    claim: ClaimRead,
    children: Vec<ClaimRead>,
    effects: Vec<LocalEffect>,
  },
  Released {
    subject: Subject,
    released_claim_id: ClaimId,
    effects: Vec<LocalEffect>,
  },
  Policy {
    policy: WorkspacePolicy,
    effects: Vec<LocalEffect>,
  },
  Delivery {
    delivery: DeliveryRead,
    effects: Vec<LocalEffect>,
  },
}

impl TeamCommandValue {
  /// The local effects the server authorized with the command.
  pub fn effects(&self) -> &[LocalEffect] {
    match self {
      Self::Task { effects, .. }
      | Self::Module { effects, .. }
      | Self::Tags { effects, .. }
      | Self::Assignment { effects, .. }
      | Self::Claim { effects, .. }
      | Self::Released { effects, .. }
      | Self::Policy { effects, .. }
      | Self::Delivery { effects, .. } => effects,
    }
  }
}
