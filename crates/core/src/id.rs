use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Defines a typed UUIDv7 identifier so ids of different entities cannot be swapped.
macro_rules! define_id {
  ($(#[$doc:meta])* $name:ident) => {
    $(#[$doc])*
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct $name(Uuid);

    impl $name {
      /// A new time-ordered id.
      pub fn generate() -> Self {
        Self(Uuid::now_v7())
      }

      pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
      }

      pub const fn as_uuid(self) -> Uuid {
        self.0
      }
    }

    impl fmt::Display for $name {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
      }
    }

    impl FromStr for $name {
      type Err = uuid::Error;

      fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
      }
    }
  };
}

define_id!(
  /// An account on a shared server.
  UserId
);
define_id!(
  /// A revocable sign-in on a shared server.
  SessionId
);
define_id!(
  /// A project, i.e. one repository managed by Orchester.
  ProjectId
);
define_id!(
  /// A versioned configuration resource (agent profile, instruction, pipeline, tool).
  ResourceId
);
define_id!(
  /// One immutable published version of a resource.
  VersionId
);
define_id!(TaskId);
define_id!(
  /// One execution of a task; names its branch and worktree.
  RunId
);
define_id!(
  /// A project connection to a repository host, agent provider or MCP server.
  ConnectionId
);
define_id!(
  /// A project secret. Bodies and rows hold only this reference; the value stays in the OS keychain.
  SecretId
);
define_id!(
  /// Client-generated key that makes a create/queue/send action safe to retry.
  IdempotencyKey
);

impl TaskId {
  /// Reopening a queued task must retry its first run with the same key. Later runs use
  /// independent request keys.
  pub const fn first_run_key(self) -> IdempotencyKey {
    IdempotencyKey::from_uuid(self.as_uuid())
  }
}
define_id!(
  /// One attempt of a run; a failure routed back to an upstream step starts a new attempt.
  AttemptId
);
define_id!(
  /// One execution of a pipeline step within an attempt.
  StepExecutionId
);
define_id!(
  /// A persisted point a run can continue from after a pause or an interruption.
  CheckpointId
);
define_id!(
  /// One launch of an agent CLI for a step execution: the first, or a retry or continue.
  InvocationId
);
define_id!(
  /// One message of a task conversation, a person's or an agent's.
  MessageId
);
define_id!(
  /// One record of task or project memory.
  MemoryId
);
define_id!(
  /// One native agent conversation: the items of one step execution, shared by its invocations.
  ConversationId
);
