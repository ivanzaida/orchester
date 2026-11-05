//! Team-authority command envelopes and receipts (S4). Commands and their values are added by the stage that
//! implements each one, so the envelopes take them as type parameters: `C` a command, internally tagged by `kind`
//! (e.g. `{kind:"claim.acquire",subject}`), and `V` its value, which carries `effects: [LocalEffect]`.

use std::{fmt, num::NonZeroU64};

pub use orchester_core::sharing::{ExpectedRevision, ExpectedRevisionError, ExpectedRevisions, RevisionTarget};
use orchester_core::{
  blob::ContentHash,
  id::{ModuleId, OperationId, TaskId, WorkId},
  run::{BranchName, CommitId},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::AuthorityError;

/// What a command acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Subject {
  Task { id: TaskId },
  Module { id: ModuleId },
}

/// The run bundle a start effect runs: its content address and the bundle format's version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleRef {
  pub sha256: ContentHash,
  pub schema_version: u32,
}

/// What the server authorized the caller's computer to do once the command committed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocalEffect {
  Start {
    work_id: WorkId,
    bundle: BundleRef,
  },
  Pause {
    work_id: WorkId,
  },
  Stop {
    work_id: WorkId,
  },
  /// The runner may merge the work's module branch into base on its computer ([`super::DeliveryRead`]).
  Deliver {
    work_id: WorkId,
  },
  /// The runner may merge `branch` at `source_commit` into base at `base_commit` on its computer
  /// ([`super::ModuleDeliveryRead`]). It names no path: the runner finds the module's checkout itself.
  DeliverModule {
    module_id: ModuleId,
    branch: BranchName,
    source_commit: CommitId,
    base_commit: CommitId,
  },
  /// Braced so serde refuses unknown fields on it too; a unit variant would ignore them.
  None {},
}

/// Why a string is not an [`EventCursor`]: it must be the canonical decimal of a positive integer.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum EventCursorError {
  #[error("an event cursor cannot be empty")]
  Empty,
  #[error("an event cursor is decimal digits only, without sign, space or other characters")]
  NotDecimal,
  #[error("an event cursor has no leading zeros")]
  LeadingZero,
  #[error("an event cursor is a positive integer, not zero")]
  Zero,
  #[error("an event cursor does not fit in 64 bits")]
  OutOfRange,
}

/// The server's committed event sequence position in one project's event stream. Its wire form is a string holding
/// the canonical decimal of a positive integer (no sign, no leading zeros). Cursors order by that integer, never by
/// string order (`"9" < "10"`): a client may compare two cursors of the same project, while cursors of different
/// projects are not comparable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EventCursor(NonZeroU64);

impl EventCursor {
  pub fn new(cursor: &str) -> Result<Self, EventCursorError> {
    if cursor.is_empty() {
      return Err(EventCursorError::Empty);
    }
    if !cursor.bytes().all(|byte| byte.is_ascii_digit()) {
      return Err(EventCursorError::NotDecimal);
    }
    if cursor == "0" {
      return Err(EventCursorError::Zero);
    }
    if cursor.starts_with('0') {
      return Err(EventCursorError::LeadingZero);
    }
    // Only digits are left, so parsing fails on overflow alone.
    cursor
      .parse::<NonZeroU64>()
      .map(Self)
      .map_err(|_| EventCursorError::OutOfRange)
  }

  /// The committed sequence position this cursor names.
  pub const fn position(self) -> NonZeroU64 {
    self.0
  }
}

impl fmt::Display for EventCursor {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(formatter)
  }
}

impl TryFrom<String> for EventCursor {
  type Error = EventCursorError;

  fn try_from(cursor: String) -> Result<Self, Self::Error> {
    Self::new(&cursor)
  }
}

impl From<EventCursor> for String {
  fn from(cursor: EventCursor) -> Self {
    cursor.to_string()
  }
}

/// One command, sent again unchanged with the same `operation_id` on retry. The server keys its receipt by project,
/// account and `operation_id`; the same id with another request is `operation_mismatch`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRequest<C> {
  pub operation_id: OperationId,
  pub expected: ExpectedRevisions,
  pub command: C,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Committed {
  Committed,
}

/// A committed command's receipt: `{operation_id, state:"committed", result, cursor}`. A refusal is an
/// [`AuthorityError`] instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResult<V> {
  pub operation_id: OperationId,
  state: Committed,
  pub result: V,
  /// The stream position just after this command's events.
  pub cursor: EventCursor,
}

impl<V> CommandResult<V> {
  pub const fn committed(operation_id: OperationId, result: V, cursor: EventCursor) -> Self {
    Self {
      operation_id,
      state: Committed::Committed,
      result,
      cursor,
    }
  }
}

/// The stored outcome of an operation, read back after a lost response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationRead<V> {
  Committed {
    operation_id: OperationId,
    response: CommandResult<V>,
  },
  Rejected {
    operation_id: OperationId,
    error: AuthorityError,
  },
}

impl<V> OperationRead<V> {
  pub const fn operation_id(&self) -> OperationId {
    match self {
      Self::Committed { operation_id, .. } | Self::Rejected { operation_id, .. } => *operation_id,
    }
  }
}
