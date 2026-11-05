//! A module's open delivery request as its [`super::ModuleRead`] carries it: who asked for the module branch to reach
//! base and when, the commits it would merge, and how it is authorized. It is open from `delivery.request` until the
//! runner reports the merge or the delivery fails ([`super::ModuleDeliveryRead`] has the whole record).

use orchester_core::run::CommitId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::UserRef;

/// How an open delivery is authorized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingAuthorization {
  /// Waits for an Admin's `delivery.authorize`.
  AwaitingAdmin,
  /// An Admin's `delivery.authorize` allowed it; the merge has not been reported yet.
  Admin,
  /// The workspace's `automatic_merge` allowed it, as the request came from the module's effective claim (D7); the
  /// merge has not been reported yet.
  Automatic,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PendingDeliveryError {
  #[error("a pending delivery's source commit is its base commit: there is nothing to deliver")]
  NothingToDeliver,
}

/// `{requested_by, requested_at, source_commit, base_commit, authorization}`: `requested_at` in milliseconds since the
/// Unix epoch, `source_commit` the module branch head to deliver, `base_commit` the base commit it lands on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PendingDeliveryWire", into = "PendingDeliveryWire")]
pub struct PendingDeliveryRead {
  requested_by: UserRef,
  requested_at: i64,
  source_commit: CommitId,
  base_commit: CommitId,
  authorization: PendingAuthorization,
}

impl PendingDeliveryRead {
  pub fn new(
    requested_by: UserRef,
    requested_at: i64,
    source_commit: CommitId,
    base_commit: CommitId,
    authorization: PendingAuthorization,
  ) -> Result<Self, PendingDeliveryError> {
    if source_commit == base_commit {
      return Err(PendingDeliveryError::NothingToDeliver);
    }
    Ok(Self {
      requested_by,
      requested_at,
      source_commit,
      base_commit,
      authorization,
    })
  }

  pub const fn requested_by(&self) -> &UserRef {
    &self.requested_by
  }

  pub const fn requested_at(&self) -> i64 {
    self.requested_at
  }

  pub const fn source_commit(&self) -> &CommitId {
    &self.source_commit
  }

  pub const fn base_commit(&self) -> &CommitId {
    &self.base_commit
  }

  pub const fn authorization(&self) -> PendingAuthorization {
    self.authorization
  }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingDeliveryWire {
  requested_by: UserRef,
  requested_at: i64,
  source_commit: CommitId,
  base_commit: CommitId,
  authorization: PendingAuthorization,
}

impl TryFrom<PendingDeliveryWire> for PendingDeliveryRead {
  type Error = PendingDeliveryError;

  fn try_from(wire: PendingDeliveryWire) -> Result<Self, Self::Error> {
    Self::new(
      wire.requested_by,
      wire.requested_at,
      wire.source_commit,
      wire.base_commit,
      wire.authorization,
    )
  }
}

impl From<PendingDeliveryRead> for PendingDeliveryWire {
  fn from(read: PendingDeliveryRead) -> Self {
    Self {
      requested_by: read.requested_by,
      requested_at: read.requested_at,
      source_commit: read.source_commit,
      base_commit: read.base_commit,
      authorization: read.authorization,
    }
  }
}
