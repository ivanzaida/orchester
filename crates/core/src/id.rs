use std::{fmt, str::FromStr};

use uuid::Uuid;

/// Defines a typed UUIDv7 identifier so ids of different entities cannot be swapped.
macro_rules! define_id {
  ($(#[$doc:meta])* $name:ident) => {
    $(#[$doc])*
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
  /// Client-generated key that makes a create/queue/send action safe to retry.
  IdempotencyKey
);
