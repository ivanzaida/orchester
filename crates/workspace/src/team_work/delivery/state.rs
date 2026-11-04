//! A team delivery as the screens show it (P6): what was requested, the state the server last answered, and how the
//! merge and its report stand here.

use orchester_api::v2::{AuthorityError, AuthorityErrorCode, DeliveryState, ModuleDeliveryRead};
use orchester_core::{
  id::{ModuleId, OperationId},
  run::{BranchName, CommitId},
};
use orchester_store::{DeliveryMerge, TeamDelivery, TeamDeliveryStage};

use super::{KeptDelivery, ModuleDeliveries};
use crate::team_work::TeamWorkError;

/// Where a delivery stands, by what the server answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliveryStatus {
  /// Recorded, and sent or about to be: the server's answer is not known yet.
  Requested,
  /// Refused for want of an authorization (`delivery_not_authorized`): it needs an Admin's `delivery.authorize`, or
  /// the workspace's automatic merge.
  AwaitingAuthorization,
  /// Requested on an Admin's authorization; its merge or its report is under way here.
  Authorized { by: OperationId },
  /// Requested on the workspace's automatic merge; its merge or its report is under way here.
  AuthorizedAutomatically,
  /// The server keeps it delivered.
  Delivered,
  /// The merge failed here, or the server keeps it failed (its automatic merge withdrawn, its claim ended).
  Failed,
  /// The server refused the request or its report for another reason, kept as it came.
  Refused(AuthorityError),
}

/// A team delivery as it stands here and on the server.
#[derive(Clone, Debug)]
pub struct TeamDeliveryState {
  pub operation: OperationId,
  pub module: ModuleId,
  pub branch: BranchName,
  /// The branch of this repository the merge lands on.
  pub base_branch: BranchName,
  pub source_commit: CommitId,
  pub base_commit: CommitId,
  pub status: DeliveryStatus,
  /// The delivery as the server last answered it.
  pub server: Option<ModuleDeliveryRead>,
  /// How the merge went here, once it was made or failed.
  pub merge: Option<DeliveryMerge>,
  /// Whether the report of the merge waits to be answered.
  pub report_waiting: bool,
}

impl ModuleDeliveries {
  /// The delivery recorded under `operation`; `None` when there is none.
  pub fn delivery_state(&self, operation: OperationId) -> Result<Option<TeamDeliveryState>, TeamWorkError> {
    self
      .db()?
      .team_delivery(operation)?
      .map(|record| state(&record))
      .transpose()
  }

  /// Every delivery of the project, oldest first.
  pub fn deliveries(&self) -> Result<Vec<TeamDeliveryState>, TeamWorkError> {
    self.db()?.team_deliveries()?.iter().map(state).collect()
  }
}

fn state(record: &TeamDelivery) -> Result<TeamDeliveryState, TeamWorkError> {
  let unreadable = || TeamWorkError::UnreadableDelivery(record.operation);
  let kept: KeptDelivery = serde_json::from_str(&record.request).map_err(|_| unreadable())?;
  let server: Option<ModuleDeliveryRead> = record
    .delivery
    .as_deref()
    .map(serde_json::from_str)
    .transpose()
    .map_err(|_| unreadable())?;
  let refusal: Option<AuthorityError> = record
    .refusal
    .as_deref()
    .map(serde_json::from_str)
    .transpose()
    .map_err(|_| unreadable())?;
  let status = status(record, server.as_ref(), refusal).ok_or_else(unreadable)?;
  Ok(TeamDeliveryState {
    operation: record.operation,
    module: kept.module,
    branch: kept.branch,
    base_branch: kept.base_branch,
    source_commit: kept.source_commit,
    base_commit: kept.base_commit,
    status,
    server,
    merge: record.merge.clone(),
    report_waiting: record.stage == TeamDeliveryStage::Reporting,
  })
}

/// The status of `record` by its stage and what the server answered; `None` when the record lacks what its stage needs.
fn status(
  record: &TeamDelivery,
  server: Option<&ModuleDeliveryRead>,
  refusal: Option<AuthorityError>,
) -> Option<DeliveryStatus> {
  let refused = |error: AuthorityError| match error.code() {
    AuthorityErrorCode::DeliveryNotAuthorized => DeliveryStatus::AwaitingAuthorization,
    _ => DeliveryStatus::Refused(error),
  };
  Some(match record.stage {
    TeamDeliveryStage::Requested => DeliveryStatus::Requested,
    TeamDeliveryStage::Refused => refused(refusal?),
    // A report refused as not authorized is of a delivery the automatic merge's withdrawal failed.
    TeamDeliveryStage::ReportRefused => match refusal? {
      error if error.code() == AuthorityErrorCode::DeliveryNotAuthorized => DeliveryStatus::Failed,
      error => DeliveryStatus::Refused(error),
    },
    TeamDeliveryStage::Reporting if matches!(record.merge, Some(DeliveryMerge::Failed { .. })) => {
      DeliveryStatus::Failed
    }
    TeamDeliveryStage::Reported => match server?.state {
      DeliveryState::Delivered => DeliveryStatus::Delivered,
      DeliveryState::Failed => DeliveryStatus::Failed,
      DeliveryState::Authorized | DeliveryState::Requested => authorized(server?),
    },
    TeamDeliveryStage::Accepted | TeamDeliveryStage::Merging | TeamDeliveryStage::Reporting => authorized(server?),
  })
}

fn authorized(server: &ModuleDeliveryRead) -> DeliveryStatus {
  server
    .authorization_operation_id
    .map_or(DeliveryStatus::AuthorizedAutomatically, |by| {
      DeliveryStatus::Authorized { by }
    })
}
