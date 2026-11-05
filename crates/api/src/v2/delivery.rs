//! Delivery (S7): taking a module branch's work to the base branch. `delivery.authorize` (Admin) and
//! `delivery.request` are the server's commands, each answering `{kind:"delivery", delivery, effects}`; an authorized
//! request carries a [`super::LocalEffect::Deliver`], with which the runner merges the module branch into base on its
//! computer.
//!
//! S7c: base delivery is a module's (D3), and a module has no work or bundle, so [`ModuleDeliveryRead`] is a type of
//! its own rather than a tagged form of [`DeliveryRead`]: the task form's wire stays exactly as it was, and the two
//! field sets are disjoint (`work_id`/`bundle` against `subject`/`branch`), so with unknown fields refused a body reads
//! as one form only.

use orchester_core::{
  id::{ClaimId, ModuleId, OperationId, WorkId},
  run::{BranchName, CommitId},
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};

use super::{BundleRef, Subject, page::nullable};

/// Where a delivery stands. The server reports `delivered` only once the runner reported the merge commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
  Authorized,
  Requested,
  Delivered,
  Failed,
}

/// `{work_id, claim_id, authorization_operation_id?, source_commit, base_commit, bundle, state, revision}`: one
/// delivery of authorized work, from `source_commit` on the module branch onto `base_commit` of base.
/// `authorization_operation_id` is the `delivery.authorize` that allowed it, `null` when none did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryRead {
  pub work_id: WorkId,
  pub claim_id: ClaimId,
  #[serde(deserialize_with = "nullable")]
  pub authorization_operation_id: Option<OperationId>,
  pub source_commit: CommitId,
  pub base_commit: CommitId,
  pub bundle: BundleRef,
  pub state: DeliveryState,
  pub revision: TaskRevision,
}

/// `{subject:{kind:"module",id}, claim_id, authorization_operation_id?, branch, source_commit, base_commit, state,
/// revision}`: one delivery of a module branch to base, bound to the module's claim, with no work and no bundle.
/// `source_commit` is the module branch's head being delivered, `base_commit` the base commit it lands on; a task
/// subject is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleDeliveryRead {
  #[serde(rename = "subject", with = "module_subject")]
  pub module_id: ModuleId,
  pub claim_id: ClaimId,
  #[serde(deserialize_with = "nullable")]
  pub authorization_operation_id: Option<OperationId>,
  pub branch: BranchName,
  pub source_commit: CommitId,
  pub base_commit: CommitId,
  pub state: DeliveryState,
  pub revision: TaskRevision,
}

/// A module id on the wire as its [`Subject`], so the read names what it delivers as every command does.
mod module_subject {
  use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

  use super::{ModuleId, Subject};

  pub fn serialize<S: Serializer>(id: &ModuleId, serializer: S) -> Result<S::Ok, S::Error> {
    Subject::Module { id: *id }.serialize(serializer)
  }

  pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<ModuleId, D::Error> {
    match Subject::deserialize(deserializer)? {
      Subject::Module { id } => Ok(id),
      Subject::Task { .. } => Err(D::Error::custom("a module delivery's subject is a module")),
    }
  }
}

/// `{kind:"delivered", merge_commit}` or `{kind:"failed"}`: how the runner's merge of a module delivery went, as
/// `delivery.report` says it. Only a `delivered` report makes the delivery delivered, with its merge commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeliveryOutcome {
  Delivered {
    merge_commit: CommitId,
  },
  /// Braced so serde refuses unknown fields on it too; a unit variant would ignore them.
  Failed {},
}
