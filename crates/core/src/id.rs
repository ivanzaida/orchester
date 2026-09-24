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
