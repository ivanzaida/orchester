//! Work reports (S9): what a runner tells its team server about authorized work it carries out, and the event that
//! announces the work's new state. `work.report` is the server's command on the S4 envelopes, its payload a
//! [`WorkReport`]; it answers `{kind:"work",work,bundle,effects}`. The server fences a report by its [`AuthorityRef`]
//! and orders it by its per-run `sequence` ([`orchester_core::run::order_report`]): a delayed report is refused
//! (`claim_lost`, `sequence_regressed`) and never overwrites the state of the work's new holder, and the same report
//! sent again is answered with its original result.

pub use orchester_core::run::{ExecutionFailure, ExecutionOutcome};
use orchester_core::{
  blob::ContentHash,
  id::{ClaimId, RunId, WorkEventId, WorkId},
};
use serde::{Deserialize, Serialize};

use super::{Subject, WorkRead, page::nullable};
use crate::Delta;

/// What a shared artifact is: the kinds the desktop produces. A closed set, so a reader refuses a kind it does not
/// know; a new kind is a new variant here and in `docs/cloud-api.md`, published before any runner reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
  /// A completed run's report, its last step's result (`orchester_core::run::REPORT_ARTIFACT`).
  Report,
  /// The changes the run made, as a patch.
  Patch,
  /// A step's captured output.
  Log,
}

/// `{sha256, kind, size_bytes}`: an artifact a report shares, by the hash of its bytes. Only a secret-free shared
/// artifact, never a path of the reporting computer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
  pub sha256: ContentHash,
  pub kind: ArtifactKind,
  pub size_bytes: u64,
}

/// `{subject, claim_id, work_id?}`: the authority a command acts under, the claim the caller holds and the work under
/// it. `work_id` is always sent, `null` before the claim has work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityRef {
  pub subject: Subject,
  pub claim_id: ClaimId,
  #[serde(deserialize_with = "nullable")]
  pub work_id: Option<WorkId>,
}

/// `{authority, run_id, event_id, sequence, outcome, artifacts}`: the payload of `work.report`. `run_id` is the
/// reporting computer's run of the work, `sequence` monotonic per run, and `event_id` made once per event and repeated
/// on every retry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkReport {
  pub authority: AuthorityRef,
  pub run_id: RunId,
  pub event_id: WorkEventId,
  pub sequence: u64,
  pub outcome: ExecutionOutcome,
  pub artifacts: Vec<ArtifactRef>,
}

/// `{work}`: the work as it is now. A client replaces its cached work with it and ignores it when `work.revision` is
/// not newer than the revision it holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkChanged {
  pub work: WorkRead,
}

/// Work deltas on the v2 surface; the existing full event stream keeps its own types.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum WorkEvent {
  #[serde(rename = "work.changed")]
  Changed(Delta<WorkChanged>),
}
