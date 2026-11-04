//! A task's local identity, apart from its definition (`docs/architecture/team-authority-plan.md`, §7 "Separate local
//! SQL identities from definitions"). Runs, conversations, usage and the other records of a task name it by its
//! identity, so they stay whole when a team task's definition is not kept on this computer. A local task's identity
//! goes with its definition; a team task's names the server's task and where it came from, never its current title,
//! brief, status or configuration.

use serde::{Deserialize, Serialize};

use crate::{
  id::{ProjectId, ServerId, TaskId},
  sharing::TaskRevision,
};

/// `{server, project, task}`: a team task, its id scoped by the server project it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeamTaskRef {
  /// The server in this computer's list.
  pub server: ServerId,
  /// The project's id on that server.
  pub project: ProjectId,
  /// The task's id on that server.
  pub task: TaskId,
}

/// Who owns the task an identity stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskOwner {
  /// A task of this computer, whose definition is a local task.
  Local,
  /// A team task, with the revision it had when this computer recorded its identity.
  Team {
    task: TeamTaskRef,
    recorded_revision: TaskRevision,
  },
}

/// The local identity of a task: the id this computer's records name it by, and its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskIdentity {
  pub id: TaskId,
  pub owner: TaskOwner,
}
