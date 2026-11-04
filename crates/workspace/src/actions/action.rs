//! The typed actions the dispatcher takes, one family per domain, and the classes they fall in. An action names what a
//! person or automation means to change, with the revision it was decided on; whether it changes this computer's
//! database or goes to the team server follows from the project's owner, never from the action.

use orchester_api::v2::{AuthorityRef, PolicyRevision, Subject};
use orchester_core::{
  id::{ClaimId, ModuleId, OperationId, TagId, TaskId, UserId},
  module::ModuleDetails,
  name::Name,
  run::{BranchName, CommitId},
  sharing::TaskRevision,
  task::{TaskDetails, organization::TaskOrganization},
};

/// One action and the operation id it keeps on every retry. The id is made once per intent: sending the same intent
/// again never makes a second task, publication or run, and the same id with another action is refused.
#[derive(Clone, Debug)]
pub struct Intent {
  operation: OperationId,
  action: Action,
}

impl Intent {
  /// `action` as a new intent, with a new operation id.
  pub fn new(action: Action) -> Self {
    Self::with_operation(OperationId::generate(), action)
  }

  /// `action` under `operation`, e.g. an intent kept across a restart.
  pub const fn with_operation(operation: OperationId, action: Action) -> Self {
    Self { operation, action }
  }

  pub const fn operation(&self) -> OperationId {
    self.operation
  }

  pub const fn action(&self) -> &Action {
    &self.action
  }
}

#[derive(Clone, Debug)]
pub enum Action {
  Task(TaskAction),
  Module(ModuleAction),
  Tag(TagAction),
  Claim(ClaimAction),
  Assignment(AssignmentAction),
  Policy(PolicyAction),
  Work(WorkAction),
  Delivery(DeliveryAction),
}

/// A task's definition: its details as the task form saves them, its organization, and its module.
#[derive(Clone, Debug)]
pub enum TaskAction {
  Create {
    details: TaskDetails,
    organization: TaskOrganization,
  },
  /// The details read as `base` at `read`, changed to `edited`.
  Edit {
    task: TaskId,
    read: TaskRevision,
    base: TaskDetails,
    edited: TaskDetails,
  },
  /// The organization read as `base` at `read`, changed to `edited`.
  Organize {
    task: TaskId,
    read: TaskRevision,
    base: TaskOrganization,
    edited: TaskOrganization,
  },
  /// Puts the task read at `read` in `module`, or in none.
  Move {
    task: TaskId,
    read: TaskRevision,
    module: Option<ModuleId>,
  },
}

/// A module's definition. `read` is the module's revision on the team server; a local module has none and is checked
/// against `base` as before.
#[derive(Clone, Debug)]
pub enum ModuleAction {
  Create {
    details: ModuleDetails,
  },
  Edit {
    module: ModuleId,
    read: TaskRevision,
    base: ModuleDetails,
    edited: ModuleDetails,
  },
  /// A team module's branch, assigned by an Admin on the module read at `read`.
  AssignBranch {
    module: ModuleId,
    read: TaskRevision,
    branch: BranchName,
  },
}

/// The project's tags.
#[derive(Clone, Debug)]
pub enum TagAction {
  Create { name: Name },
  Rename { tag: TagId, name: Name },
  Merge { tag: TagId, into: TagId },
  Delete { tag: TagId },
}

/// Picking up and dropping a team task or module (`claim.*`).
#[derive(Clone, Debug)]
pub enum ClaimAction {
  Acquire {
    subject: Subject,
  },
  /// Takes over the stale claim `observed`, which the caller saw on the subject.
  TakeOver {
    subject: Subject,
    observed: ClaimId,
  },
  Release {
    authority: AuthorityRef,
  },
}

/// Assigns a team task or module read at `read` to `user`, or to nobody (Admin).
#[derive(Clone, Copy, Debug)]
pub struct AssignmentAction {
  pub subject: Subject,
  pub read: TaskRevision,
  pub user: Option<UserId>,
}

/// The workspace's automatic merge, read at policy revision `read` (Admin).
#[derive(Clone, Copy, Debug)]
pub struct PolicyAction {
  pub read: PolicyRevision,
  pub automatic_merge: bool,
}

/// Starts a team task's work under the task's claim `authority`, from the base branch at `base_commit`, on the task
/// read at `read` in `module` read at `module_read`.
#[derive(Clone, Debug)]
pub struct WorkAction {
  pub authority: AuthorityRef,
  pub base_commit: CommitId,
  pub read: TaskRevision,
  pub module: ModuleId,
  pub module_read: TaskRevision,
}

/// Authorizes the delivery of `module`'s `branch` under the module's claim `claim`, from `source_commit` onto the base
/// at `base_commit` (Admin).
#[derive(Clone, Debug)]
pub struct DeliveryAction {
  pub module: ModuleId,
  pub claim: ClaimId,
  pub branch: BranchName,
  pub source_commit: CommitId,
  pub base_commit: CommitId,
}

/// How an action is carried out (`docs/architecture/team-authority-plan.md` §4, "Distinguish action classes").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionClass {
  /// Changes this computer's database only: an action on a local project.
  LocalOnly,
  /// The team server commits it; this computer only learns of it.
  ServerOnly,
  /// The team server commits it and its receipt allows this computer to start work: `work.start`.
  ServerThenLocalExecution,
  /// Pausing, stopping or answering within work already authorized, reported afterwards. No P2 action is of this
  /// class: the work commands join it in P4.
  AuthorizedLocalExecution,
  /// An execution event kept here and delivered to the server. No P2 action is of this class: `work.report` joins it
  /// in P4.
  ExecutionReporting,
}

impl Action {
  /// Whether the action exists only for a team project: claims, assignment, the policy, work, delivery and module
  /// branches.
  pub const fn team_only(&self) -> bool {
    matches!(
      self,
      Self::Claim(_)
        | Self::Assignment(_)
        | Self::Policy(_)
        | Self::Work(_)
        | Self::Delivery(_)
        | Self::Module(ModuleAction::AssignBranch { .. })
    )
  }

  /// The class of the action on a project that is a team's when `team` is set. A claim allows no local effect on the
  /// team server: only the work it starts does.
  pub const fn class(&self, team: bool) -> ActionClass {
    match (team, self) {
      (false, _) => ActionClass::LocalOnly,
      (true, Self::Work(_)) => ActionClass::ServerThenLocalExecution,
      (true, _) => ActionClass::ServerOnly,
    }
  }
}
