//! Who holds a task or module and what runs for it (S10): the effective claim and the authorized work, as task and
//! module reads carry them. Times are milliseconds since the Unix epoch, as everywhere in the public API.

use orchester_core::{
  id::{ClaimId, RunId, WorkId},
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};

use super::{BundleRef, Subject, page::nullable};
use crate::UserRef;

/// `{subject, id, holder, acquired_at, last_execution_heartbeat_at, stale_at, stale, parent_module_claim_id?}`: a claim
/// as the server keeps it. A task's claim taken under its module's claim names that one in `parent_module_claim_id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRead {
  pub subject: Subject,
  pub id: ClaimId,
  pub holder: UserRef,
  pub acquired_at: i64,
  pub last_execution_heartbeat_at: i64,
  /// When the claim goes stale without another heartbeat, by the server's claim staleness threshold.
  pub stale_at: i64,
  pub stale: bool,
  #[serde(deserialize_with = "nullable")]
  pub parent_module_claim_id: Option<ClaimId>,
}

/// Where authorized work stands. The proposed set of the contract: which human waits (a question, a plan, an approval)
/// have states of their own is an open team-authority decision, and until it is made every one is `awaiting_human`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
  Running,
  Paused,
  AwaitingHuman,
  Succeeded,
  Failed,
  Cancelled,
}

/// `{id, subject, claim_id, revision, state, runs, bundle, created_at, updated_at}`: one unit of work the server
/// authorized under a claim, with the runs that carried it out, oldest first. Its state is the work's execution, apart
/// from the task's workflow status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkRead {
  pub id: WorkId,
  pub subject: Subject,
  pub claim_id: ClaimId,
  pub revision: TaskRevision,
  pub state: ExecutionState,
  pub runs: Vec<RunId>,
  pub bundle: BundleRef,
  pub created_at: i64,
  pub updated_at: i64,
}
