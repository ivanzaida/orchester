//! Team module deliveries (`schema::team_delivery`): a `delivery.request` of a team module's branch, recorded by its
//! operation before the command is sent and carried forward one boundary at a time, so a crash at any of them goes on
//! from it once: the local merge happens at most once, and its report is made once and sent until the server answers
//! it. The request, the server's deliveries and refusals and the report's request are JSON the workspace writes and
//! reads; the store keeps them as they are and holds the order of the boundaries.

use orchester_core::{id::OperationId, run::CommitId, time::Timestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// Where a team delivery stands on this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamDeliveryStage {
  /// Recorded before `delivery.request` was sent: whether the server took it is not known yet.
  Requested,
  /// The server refused the request; nothing is merged.
  Refused,
  /// The server took the request with its `deliver_module` effect: the merge is this computer's to make.
  Accepted,
  /// The merge began: git may have made it before the computer stopped.
  Merging,
  /// The merge's outcome is kept with the report made of it, which waits for the server's answer.
  Reporting,
  /// The server answered the report.
  Reported,
  /// The server refused the report; its refusal is kept.
  ReportRefused,
}

impl TeamDeliveryStage {
  const fn text(self) -> &'static str {
    match self {
      Self::Requested => "requested",
      Self::Refused => "refused",
      Self::Accepted => "accepted",
      Self::Merging => "merging",
      Self::Reporting => "reporting",
      Self::Reported => "reported",
      Self::ReportRefused => "report_refused",
    }
  }

  fn parse(text: &str) -> Result<Self, StoreError> {
    Ok(match text {
      "requested" => Self::Requested,
      "refused" => Self::Refused,
      "accepted" => Self::Accepted,
      "merging" => Self::Merging,
      "reporting" => Self::Reporting,
      "reported" => Self::Reported,
      "report_refused" => Self::ReportRefused,
      other => return Err(corrupt("team delivery stage", other)),
    })
  }

  /// Whether the delivery goes no further.
  pub const fn settled(self) -> bool {
    matches!(self, Self::Refused | Self::Reported | Self::ReportRefused)
  }
}

/// How the local merge of a delivery went, kept with its report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliveryMerge {
  /// It merged; the base branch is at `commit`.
  Merged { commit: CommitId },
  /// It did not merge, and why, as the workspace wrote it.
  Failed { failure: String },
}

/// The report of a delivery's merge, made once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryReport {
  pub operation: OperationId,
  pub request: String,
}

/// A recorded team delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamDelivery {
  pub operation: OperationId,
  pub request: String,
  pub stage: TeamDeliveryStage,
  /// The delivery as the server last answered it, from [`TeamDeliveryStage::Accepted`] on.
  pub delivery: Option<String>,
  /// The server's refusal of the request or of the report, as it came.
  pub refusal: Option<String>,
  /// The merge's outcome and its report, from [`TeamDeliveryStage::Reporting`] on.
  pub merge: Option<DeliveryMerge>,
  pub report: Option<DeliveryReport>,
}

impl ProjectDb {
  /// Records the delivery requested under `operation` with `request` before `delivery.request` is sent, and returns
  /// it. The same operation recorded before with the same request returns that record, wherever it stands; with
  /// another one it is [`StoreError::OperationMismatch`].
  pub fn begin_team_delivery(
    &mut self,
    operation: OperationId,
    request: &str,
    at: Timestamp,
  ) -> Result<TeamDelivery, StoreError> {
    let transaction = self.connection.transaction()?;
    if let Some(known) = load(&transaction, operation)? {
      if known.request != request {
        return Err(StoreError::OperationMismatch(operation));
      }
      return Ok(known);
    }
    transaction.execute(
      "INSERT INTO team_deliveries (operation_id, request, stage, created_at, updated_at) \
       VALUES (?1, ?2, 'requested', ?3, ?3)",
      params![operation.to_string(), request, at.unix_millis()],
    )?;
    let recorded = load(&transaction, operation)?.ok_or(StoreError::TeamDeliveryNotFound(operation))?;
    transaction.commit()?;
    Ok(recorded)
  }

  pub fn team_delivery(&self, operation: OperationId) -> Result<Option<TeamDelivery>, StoreError> {
    load(&self.connection, operation)
  }

  /// Every team delivery of the project, oldest first.
  pub fn team_deliveries(&self) -> Result<Vec<TeamDelivery>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("{SELECT} ORDER BY created_at, operation_id"))?;
    let rows = statement.query_map([], read)?;
    rows.map(|row| row?).collect()
  }

  /// Records the server's `refusal` of the request under `operation`, kept as it came.
  pub fn refuse_team_delivery(
    &mut self,
    operation: OperationId,
    refusal: &str,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE team_deliveries SET stage = 'refused', refusal = ?2, updated_at = ?3 WHERE operation_id = ?1 \
       AND stage = 'requested'",
      params![operation.to_string(), refusal, at.unix_millis()],
    )?;
    ensure_updated(&self.connection, operation, updated)
  }

  /// Keeps the `delivery` the receipt of `operation` answered with, whose effect lets this computer merge. A delivery
  /// accepted already stays as it is.
  pub fn accept_team_delivery(
    &mut self,
    operation: OperationId,
    delivery: &str,
    at: Timestamp,
  ) -> Result<TeamDelivery, StoreError> {
    let transaction = self.connection.transaction()?;
    let known = load(&transaction, operation)?.ok_or(StoreError::TeamDeliveryNotFound(operation))?;
    match known.stage {
      TeamDeliveryStage::Requested => {
        transaction.execute(
          "UPDATE team_deliveries SET stage = 'accepted', delivery = ?2, updated_at = ?3 WHERE operation_id = ?1",
          params![operation.to_string(), delivery, at.unix_millis()],
        )?;
      }
      TeamDeliveryStage::Refused => return Err(out_of_order(&known)),
      _ => return Ok(known),
    }
    let accepted = load(&transaction, operation)?.ok_or(StoreError::TeamDeliveryNotFound(operation))?;
    transaction.commit()?;
    Ok(accepted)
  }

  /// Marks the accepted delivery under `operation` merging, before git is asked to merge.
  pub fn begin_team_delivery_merge(&mut self, operation: OperationId, at: Timestamp) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE team_deliveries SET stage = 'merging', updated_at = ?2 WHERE operation_id = ?1 AND stage = 'accepted'",
      params![operation.to_string(), at.unix_millis()],
    )?;
    ensure_updated(&self.connection, operation, updated)
  }

  /// Keeps how the merge of the delivery under `operation` went with the `report` made of it, in one transaction.
  pub fn queue_team_delivery_report(
    &mut self,
    operation: OperationId,
    merge: &DeliveryMerge,
    report: &DeliveryReport,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let (commit, failure) = match merge {
      DeliveryMerge::Merged { commit } => (Some(commit.as_str()), None),
      DeliveryMerge::Failed { failure } => (None, Some(failure.as_str())),
    };
    let updated = self.connection.execute(
      "UPDATE team_deliveries SET stage = 'reporting', merge_commit = ?2, failure = ?3, report_operation_id = ?4, \
       report_request = ?5, updated_at = ?6 WHERE operation_id = ?1 AND stage = 'merging'",
      params![
        operation.to_string(),
        commit,
        failure,
        report.operation.to_string(),
        report.request,
        at.unix_millis()
      ],
    )?;
    ensure_updated(&self.connection, operation, updated)
  }

  /// Records the server's answer to the report of the delivery under `operation`: the delivery as it is now.
  pub fn acknowledge_team_delivery_report(
    &mut self,
    operation: OperationId,
    delivery: &str,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE team_deliveries SET stage = 'reported', delivery = ?2, updated_at = ?3 WHERE operation_id = ?1 \
       AND stage = 'reporting'",
      params![operation.to_string(), delivery, at.unix_millis()],
    )?;
    ensure_updated(&self.connection, operation, updated)
  }

  /// Records the server's `refusal` of the report of the delivery under `operation`, kept as it came.
  pub fn refuse_team_delivery_report(
    &mut self,
    operation: OperationId,
    refusal: &str,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE team_deliveries SET stage = 'report_refused', refusal = ?2, updated_at = ?3 WHERE operation_id = ?1 \
       AND stage = 'reporting'",
      params![operation.to_string(), refusal, at.unix_millis()],
    )?;
    ensure_updated(&self.connection, operation, updated)
  }
}

/// The delivery out of order when `updated` rows say the update found it elsewhere.
fn ensure_updated(connection: &Connection, operation: OperationId, updated: usize) -> Result<(), StoreError> {
  if updated == 1 {
    return Ok(());
  }
  let known = load(connection, operation)?.ok_or(StoreError::TeamDeliveryNotFound(operation))?;
  Err(out_of_order(&known))
}

const fn out_of_order(delivery: &TeamDelivery) -> StoreError {
  StoreError::TeamDeliveryStage {
    operation: delivery.operation,
    stage: delivery.stage.text(),
  }
}

const SELECT: &str = "SELECT operation_id, request, stage, delivery, refusal, merge_commit, failure, \
                      report_operation_id, report_request FROM team_deliveries";

fn load(connection: &Connection, operation: OperationId) -> Result<Option<TeamDelivery>, StoreError> {
  connection
    .query_row(
      &format!("{SELECT} WHERE operation_id = ?1"),
      [operation.to_string()],
      read,
    )
    .optional()?
    .transpose()
}

/// A row as a delivery; a value that breaks the rules is [`StoreError::Corrupt`].
fn read(row: &Row<'_>) -> rusqlite::Result<Result<TeamDelivery, StoreError>> {
  let (operation, request, stage): (String, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
  let (delivery, refusal, commit, failure): (Option<String>, Option<String>, Option<String>, Option<String>) =
    (row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?);
  let report: (Option<String>, Option<String>) = (row.get(7)?, row.get(8)?);
  Ok((|| {
    let merge = match (commit, failure) {
      (Some(commit), None) => Some(DeliveryMerge::Merged {
        commit: CommitId::new(&commit).map_err(|error| corrupt("team delivery merge commit", error))?,
      }),
      (None, Some(failure)) => Some(DeliveryMerge::Failed { failure }),
      (None, None) => None,
      (Some(_), Some(_)) => return Err(corrupt("team delivery merge", "merged and failed")),
    };
    let report = match report {
      (Some(operation), Some(request)) => Some(DeliveryReport {
        operation: parse("team delivery report operation", &operation)?,
        request,
      }),
      (None, None) => None,
      _ => return Err(corrupt("team delivery report", "an operation without its request")),
    };
    Ok(TeamDelivery {
      operation: parse("team delivery operation", &operation)?,
      request,
      stage: TeamDeliveryStage::parse(&stage)?,
      delivery,
      refusal,
      merge,
      report,
    })
  })())
}
