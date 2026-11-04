//! The team commands of `POST /api/v2/projects/{project}/commands` on the S4 envelopes (`CommandRequest<TeamCommand>`,
//! answered with `CommandResult<TeamCommandValue>`), field for field the team server's own `Command` and
//! `CommandValue` (`docs/cloud-api.md`, "Team authority P2 commands" and "Team authority P4 work"), so the server can
//! take these types without a wire change. Each is internally tagged by its `kind`, and unknown fields are refused.
//! `delivery.request` and `delivery.report` are the server's too and join with the same shapes when the desktop
//! delivers. The definitions (task edits, modules, tags, the workspace policy) are no commands: they have their own
//! admin-checked routes. The task in a value is the public `SharedTask` until the canonical `TaskRead` is published
//! there.

use orchester_core::{
  id::{ClaimId, ModuleId, UserId},
  run::{BranchName, CommitId},
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};

use super::{
  AuthorityRef, BundleRef, ClaimRead, LocalEffect, ModuleDeliveryRead, ModuleRead, Subject, WorkRead, WorkReport,
  page::nullable,
};
use crate::{SharedTask, TaskContent, UserRef};

/// A team command, as `CommandRequest::command` carries it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum TeamCommand {
  /// One task, numbered by the server (Admin).
  #[serde(rename = "task.create")]
  TaskCreate { content: TaskContent },
  /// Picks the task, or the module with all its tasks, up.
  #[serde(rename = "claim.acquire")]
  ClaimAcquire { subject: Subject },
  /// Replaces the stale claim the caller saw.
  #[serde(rename = "claim.take_over")]
  ClaimTakeOver {
    subject: Subject,
    observed_claim_id: ClaimId,
  },
  /// Drops exactly that claim, a module's with its tasks' claims.
  #[serde(rename = "claim.release")]
  ClaimRelease { authority: AuthorityRef },
  /// Asks a member to take the task or module, or nobody (`null`); unassigning the holder ends their claim (Admin).
  #[serde(rename = "assignment.set")]
  AssignmentSet {
    subject: Subject,
    #[serde(deserialize_with = "nullable")]
    user_id: Option<UserId>,
  },
  /// Gives the module another branch, decided against the module's expected revision (Admin).
  #[serde(rename = "module.branch.set")]
  ModuleBranchSet { module_id: ModuleId, branch: BranchName },
  /// Starts the task's work on its module's branch under the task's current claim, from the base branch's commit the
  /// runner read; decided against the task's and its module's expected revisions.
  #[serde(rename = "work.start")]
  WorkStart {
    authority: AuthorityRef,
    base_commit: CommitId,
  },
  /// Authorizes exactly this delivery of a module's branch to the base (Admin).
  #[serde(rename = "delivery.authorize")]
  DeliveryAuthorize {
    subject: Subject,
    claim_id: ClaimId,
    branch: BranchName,
    source_commit: CommitId,
    base_commit: CommitId,
  },
  /// A run's account of the work it carries out under the work's exact authority, ordered per run.
  #[serde(rename = "work.report")]
  WorkReport(WorkReport),
}

impl TeamCommand {
  /// The command's `kind` on the wire, e.g. `claim.acquire`.
  pub const fn kind(&self) -> &'static str {
    match self {
      Self::TaskCreate { .. } => "task.create",
      Self::ClaimAcquire { .. } => "claim.acquire",
      Self::ClaimTakeOver { .. } => "claim.take_over",
      Self::ClaimRelease { .. } => "claim.release",
      Self::AssignmentSet { .. } => "assignment.set",
      Self::ModuleBranchSet { .. } => "module.branch.set",
      Self::WorkStart { .. } => "work.start",
      Self::DeliveryAuthorize { .. } => "delivery.authorize",
      Self::WorkReport(_) => "work.report",
    }
  }
}

/// What a committed team command answers, as `CommandResult::result` carries it. `effects` lists what the caller's
/// computer may do now: a started work's `start`; every other value's list is empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TeamCommandValue {
  /// `task.create`.
  Task {
    task: Box<SharedTask>,
    effects: Vec<LocalEffect>,
  },
  /// `work.start`: the work it made, with the bundle it runs; `work.report`: the work as the report left it.
  Work {
    work: Box<WorkRead>,
    bundle: BundleRef,
    effects: Vec<LocalEffect>,
  },
  /// `claim.acquire`, `claim.take_over`: the claim the caller holds now; a module's lists its tasks' claims.
  Claim {
    subject: Subject,
    claim: Box<ClaimRead>,
    children: Vec<ClaimRead>,
    effects: Vec<LocalEffect>,
  },
  Released {
    subject: Subject,
    released_claim_id: ClaimId,
    effects: Vec<LocalEffect>,
  },
  /// The assignee as it is now, and the subject's revision, which assignment does not move.
  Assignment {
    subject: Subject,
    assignee: Option<UserRef>,
    revision: TaskRevision,
    effects: Vec<LocalEffect>,
  },
  /// `module.branch.set`: the module as it is now, with its branch.
  Module {
    module: Box<ModuleRead>,
    effects: Vec<LocalEffect>,
  },
  /// `delivery.authorize`: the module's delivery as it is after the command.
  Delivery {
    delivery: Box<ModuleDeliveryRead>,
    effects: Vec<LocalEffect>,
  },
}

impl TeamCommandValue {
  /// The local effects the server authorized with the command.
  pub fn effects(&self) -> &[LocalEffect] {
    match self {
      Self::Task { effects, .. }
      | Self::Work { effects, .. }
      | Self::Claim { effects, .. }
      | Self::Released { effects, .. }
      | Self::Assignment { effects, .. }
      | Self::Module { effects, .. }
      | Self::Delivery { effects, .. } => effects,
    }
  }
}
