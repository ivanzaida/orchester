//! Delivery (S7): taking a module branch's work to the base branch. `delivery.authorize` (Admin) and
//! `delivery.request` are the server's commands, each answering `{kind:"delivery", delivery, effects}`; an authorized
//! request carries a [`super::LocalEffect::Deliver`], with which the runner merges the module branch into base on its
//! computer.

use orchester_core::{
  id::{ClaimId, OperationId, WorkId},
  run::CommitId,
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};

use super::{BundleRef, page::nullable};

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
