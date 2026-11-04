//! The team-authority refusal: `{operation_id?, code, details}`. Details are closed variants that never echo the
//! request or carry credentials.

use orchester_core::id::OperationId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityErrorCode {
  Unauthenticated,
  NotFound,
  Forbidden,
  Invalid,
  RevisionConflict,
  /// The operation id was used before with another request.
  OperationMismatch,
  ClaimConflict,
  /// The claim or work the request names is no longer the effective one: it was released, taken over or fenced by a
  /// newer generation. A delayed work report gets this, never a write over the new holder's state.
  ClaimLost,
  ModuleRequired,
  ModuleClaimConflict,
  TransitionDenied,
  /// Carries the open decisions ([`ErrorDetails::DecisionRequired`]).
  DecisionRequired,
  BundleMissing,
  DeliveryNotAuthorized,
  /// The claim is the caller's, but held by another of their sessions than the one that sent the request.
  SessionMismatch,
  /// A work report whose sequence is not after the last one accepted for its run (`orchester_core::run::order_report`).
  /// A duplicate of the last accepted report is not refused: it is answered with the original result.
  SequenceRegressed,
  /// The caller is no longer a member of the workspace.
  MembershipRevoked,
  CursorExpired,
  UnsupportedContract,
  RateLimited,
  Unavailable,
}

impl AuthorityErrorCode {
  /// The HTTP status a response with this code has.
  pub const fn status(self) -> u16 {
    match self {
      Self::Unauthenticated => 401,
      Self::NotFound => 404,
      Self::Forbidden | Self::DeliveryNotAuthorized | Self::MembershipRevoked => 403,
      Self::Invalid => 422,
      Self::RevisionConflict
      | Self::OperationMismatch
      | Self::ClaimConflict
      | Self::ClaimLost
      | Self::ModuleRequired
      | Self::ModuleClaimConflict
      | Self::TransitionDenied
      | Self::DecisionRequired
      | Self::BundleMissing
      | Self::SessionMismatch
      | Self::SequenceRegressed => 409,
      Self::CursorExpired => 410,
      Self::UnsupportedContract => 426,
      Self::RateLimited => 429,
      Self::Unavailable => 503,
    }
  }
}

/// The closed set of team-authority decisions, D1 to D9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionId {
  D1,
  D2,
  D3,
  D4,
  D5,
  D6,
  D7,
  D8,
  D9,
}

/// The details of a refusal. Every code but `decision_required` has none (`{}` on the wire).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorDetails {
  None,
  DecisionRequired { decisions: Vec<DecisionId> },
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AuthorityErrorShapeError {
  #[error("the details of `{0:?}` do not belong to that code")]
  DetailsMismatch(AuthorityErrorCode),
  #[error("decision_required names no decision")]
  NoDecisions,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Wire", into = "Wire")]
pub struct AuthorityError {
  operation_id: Option<OperationId>,
  code: AuthorityErrorCode,
  details: ErrorDetails,
}

impl AuthorityError {
  /// `operation_id` is absent when the refusal came before the request's operation was read (e.g. unauthenticated).
  pub fn new(
    operation_id: Option<OperationId>,
    code: AuthorityErrorCode,
    details: ErrorDetails,
  ) -> Result<Self, AuthorityErrorShapeError> {
    let decision_details = matches!(details, ErrorDetails::DecisionRequired { .. });
    if decision_details != (code == AuthorityErrorCode::DecisionRequired) {
      return Err(AuthorityErrorShapeError::DetailsMismatch(code));
    }
    if matches!(&details, ErrorDetails::DecisionRequired { decisions } if decisions.is_empty()) {
      return Err(AuthorityErrorShapeError::NoDecisions);
    }
    Ok(Self {
      operation_id,
      code,
      details,
    })
  }

  pub const fn operation_id(&self) -> Option<OperationId> {
    self.operation_id
  }

  pub const fn code(&self) -> AuthorityErrorCode {
    self.code
  }

  pub const fn details(&self) -> &ErrorDetails {
    &self.details
  }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  operation_id: Option<OperationId>,
  code: AuthorityErrorCode,
  details: DetailsWire,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DetailsWire {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  decisions: Option<Vec<DecisionId>>,
}

impl TryFrom<Wire> for AuthorityError {
  type Error = AuthorityErrorShapeError;

  fn try_from(wire: Wire) -> Result<Self, Self::Error> {
    let details = match wire.details.decisions {
      Some(decisions) => ErrorDetails::DecisionRequired { decisions },
      None => ErrorDetails::None,
    };
    Self::new(wire.operation_id, wire.code, details)
  }
}

impl From<AuthorityError> for Wire {
  fn from(error: AuthorityError) -> Self {
    let decisions = match error.details {
      ErrorDetails::DecisionRequired { decisions } => Some(decisions),
      ErrorDetails::None => None,
    };
    Self {
      operation_id: error.operation_id,
      code: error.code,
      details: DetailsWire { decisions },
    }
  }
}
