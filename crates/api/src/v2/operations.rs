//! Team-authority command envelopes and receipts (S4). Commands and their values are added by the stage that
//! implements each one, so the envelopes take them as type parameters: `C` a command, internally tagged by `kind`
//! (e.g. `{kind:"claim.acquire",subject}`), and `V` its value, which carries `effects: [LocalEffect]`.

pub use orchester_core::sharing::{ExpectedRevision, ExpectedRevisionError, ExpectedRevisions, RevisionTarget};
use orchester_core::{
  blob::ContentHash,
  id::{ModuleId, OperationId, TaskId, WorkId},
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
  /// Braced so serde refuses unknown fields on it too; a unit variant would ignore them.
  None {},
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("an event cursor cannot be empty")]
pub struct EventCursorError;

/// A server-made position in the project's event stream. Opaque: clients store and send it back, never read it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EventCursor(String);

impl EventCursor {
  pub fn new(cursor: String) -> Result<Self, EventCursorError> {
    if cursor.is_empty() {
      return Err(EventCursorError);
    }
    Ok(Self(cursor))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for EventCursor {
  type Error = EventCursorError;

  fn try_from(cursor: String) -> Result<Self, Self::Error> {
    Self::new(cursor)
  }
}

impl From<EventCursor> for String {
  fn from(cursor: EventCursor) -> Self {
    cursor.0
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
