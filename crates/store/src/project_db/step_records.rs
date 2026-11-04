//! What the runner records of a step's work beside its log and transcript, as values that can be sent
//! (`docs/architecture/remote-runners.md`, rule 2): an execution's output, its progress line, what a review cost and
//! the plan limits a CLI reported. Each record holds the columns its write binds, and [`ProjectDb::write_step`] writes
//! it with the statement and in the transaction the direct writes (`save_step_output`, `report_step_progress`,
//! `record_reviewer_usage`, `save_limit_snapshot`) use, as they go through it too.

use orchester_core::{
  connection::plan_limits::LimitReading,
  id::{RunId, StepExecutionId},
  run::{StepOutput, StepProgress, review::ReviewRecord},
  time::Timestamp,
};
use serde::{Deserialize, Serialize};

use super::{
  ProjectDb, plan_limits::SnapshotRow, reviewer_usage::ReviewRow, step_outputs::OutputRow, step_progress::ProgressRow,
  write_transaction::retried,
};
use crate::StoreError;

/// A record of a step's work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRecord(StepChange);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum StepChange {
  /// Appended to `step_outputs`; an execution that has one keeps it.
  Output { row: OutputRow },
  /// Replaces the execution's line in `step_progress`.
  Progress { row: ProgressRow },
  /// Appended to `reviewer_usage`.
  Review { row: Box<ReviewRow> },
  /// Appended to `plan_limit_snapshots`, the reports older than 90 days before it removed.
  Limits { row: SnapshotRow },
}

impl StepRecord {
  /// `output`, what its execution produced; a question is no output, and is refused.
  pub fn output(output: &StepOutput) -> Result<Self, StoreError> {
    Ok(Self(StepChange::Output {
      row: OutputRow::of(output)?,
    }))
  }

  /// `progress` as the line of `execution`, reported at `at`.
  pub fn progress(execution: StepExecutionId, progress: &StepProgress, at: Timestamp) -> Self {
    Self(StepChange::Progress {
      row: ProgressRow::of(execution, progress, at),
    })
  }

  /// What one review cost.
  pub fn review(record: &ReviewRecord) -> Result<Self, StoreError> {
    Ok(Self(StepChange::Review {
      row: Box::new(ReviewRow::of(record)?),
    }))
  }

  /// What `run`'s CLI reported of its limits.
  pub fn limits(run: RunId, reading: &LimitReading) -> Result<Self, StoreError> {
    Ok(Self(StepChange::Limits {
      row: SnapshotRow::of(run, reading)?,
    }))
  }
}

impl ProjectDb {
  /// Writes `record` as its direct write does.
  pub fn write_step(&mut self, record: &StepRecord) -> Result<(), StoreError> {
    match &record.0 {
      StepChange::Output { row } => retried(&mut self.connection, "save step output", |transaction| {
        row.save(transaction)
      }),
      StepChange::Progress { row } => row.save(&self.connection),
      StepChange::Review { row } => row.insert(&self.connection),
      StepChange::Limits { row } => row.save(self),
    }
  }
}
