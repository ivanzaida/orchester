//! What a run reports to its team server about the authorized work it carries out (S9 `work.report`), and the rule
//! that orders those reports. An execution success is not permission to mark the task Done: the human-wait and
//! task-completion effects wait for the team-authority decisions D1, D4 and D8 and have no outcome here.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::status::RunFailure;

/// Why authorized work failed, as the desktop's run states name it. A closed set of kinds without text, paths or local
/// ids, so a report never carries a secret or one computer's details.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionFailure {
  /// A step failed and its pipeline step has no failure route ([`RunFailure::StepFailed`]).
  StepFailed,
  /// A step failed after its failure route was taken as often as allowed ([`RunFailure::RetriesExhausted`]).
  RetriesExhausted,
  /// The run never started: its worktree could not be created ([`RunFailure::WorktreeNotCreated`]).
  WorktreeNotCreated,
  /// The run stopped at a limit: its tokens, time, a step's budget, the daily or group budget, or the agent account's
  /// plan ([`super::RunLimit`]).
  LimitReached,
  /// The runner could not record or carry on the run ([`super::InterruptCause::RunnerFailed`]).
  RunnerFailed,
}

impl From<&RunFailure> for ExecutionFailure {
  fn from(failure: &RunFailure) -> Self {
    match failure {
      RunFailure::StepFailed(_) => Self::StepFailed,
      RunFailure::RetriesExhausted(_) => Self::RetriesExhausted,
      RunFailure::WorktreeNotCreated(_) => Self::WorktreeNotCreated,
    }
  }
}

/// `{kind:"progress"}`, `{kind:"succeeded"}`, `{kind:"failed",reason}` or `{kind:"cancelled"}`: where the reporting
/// run's execution stands. Braced variants, so serde refuses unknown fields on each of them too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionOutcome {
  /// The run goes on.
  Progress {},
  Succeeded {},
  Failed {
    reason: ExecutionFailure,
  },
  Cancelled {},
}

/// How a report stands against the last report the server accepted for the same run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportOrder {
  /// After the last accepted one: the server applies it.
  Next,
  /// The last accepted report again, with the same sequence and content: answered with its original result.
  Duplicate,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("report {sequence} is not after the last accepted report {last}")]
pub struct SequenceRegressed {
  pub last: u64,
  pub sequence: u64,
}

/// Orders `report`, numbered `sequence`, after `last`, the last report accepted for the same run with its sequence
/// (`None` before the first). Sequences are monotonic per run: only a greater one is accepted, an equal one with the
/// same content is a duplicate, and any other is refused, so a delayed report never overwrites newer state.
pub fn order_report<R: PartialEq>(
  last: Option<(u64, &R)>,
  sequence: u64,
  report: &R,
) -> Result<ReportOrder, SequenceRegressed> {
  match last {
    None => Ok(ReportOrder::Next),
    Some((last, _)) if sequence > last => Ok(ReportOrder::Next),
    Some((last, accepted)) if sequence == last && accepted == report => Ok(ReportOrder::Duplicate),
    Some((last, _)) => Err(SequenceRegressed { last, sequence }),
  }
}
