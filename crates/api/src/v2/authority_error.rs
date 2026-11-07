//! The team-authority refusal: `{operation_id?, code, details}`. Details are closed variants that never echo the
//! request or carry credentials.

use orchester_core::{
  id::{OperationId, ResourceId},
  resource::VersionNumber,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::releases::ReleaseVersion;

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
  /// `work.start` could not build the work's execution bundle. Carries why, when the server says
  /// ([`ErrorDetails::BundleMissing`]).
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
  /// Carries the oldest desktop version the server accepts ([`ErrorDetails::UnsupportedContract`]).
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

/// What the team server could not build a bundle from: the closed set of reasons a `bundle_missing` can name. A kind
/// this version does not know reads as [`Self::Other`], so a server that learns a reason first is still understood.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleMissingKind {
  /// The task's pipeline: `resource_id` is the pipeline.
  PipelineMissing,
  /// A file a pinned version holds is no longer there.
  FileMissing,
  /// An agent profile still in the old local form, with no agent CLI kind: `resource_id` is the profile, to be
  /// published again as a team profile.
  CliKindMissing,
  /// An agent profile names MCP servers that no team MCP server stands for: `resource_id` is the profile, to be
  /// published again as a team profile.
  McpServersUnresolved,
  /// A profile names a team MCP server that is archived: `resource_id` is the archived server.
  McpServerArchived,
  /// A pinned version's body cannot be read.
  BodyUnreadable,
  /// The bundle would be larger than the server builds.
  BundleTooLarge,
  #[serde(other)]
  Other,
}

/// `{kind, resource_id?, version?}`: why a `bundle_missing` refusal built no bundle. It names no path and carries no
/// secret. Unlike the closed shapes around it, it ignores fields it does not know, so a reason that grows a field is
/// still read by this version.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BundleMissingReason {
  pub kind: BundleMissingKind,
  /// The resource the kind is about, when it is about one.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub resource_id: Option<ResourceId>,
  /// The version of that resource, when the kind is about one.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub version: Option<VersionNumber>,
}

/// The details of a refusal. Every code but `decision_required`, `unsupported_contract` and `bundle_missing` has none
/// (`{}` on the wire), and `bundle_missing` has none when the server does not say why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorDetails {
  None,
  DecisionRequired {
    decisions: Vec<DecisionId>,
  },
  /// `{required_version}`: the oldest desktop version the server accepts, so the desktop can name the update it needs.
  UnsupportedContract {
    required_version: ReleaseVersion,
  },
  /// `{reason}`: why the bundle could not be built.
  BundleMissing {
    reason: BundleMissingReason,
  },
}

impl ErrorDetails {
  /// The one code these details belong to, `None` for the details every other code has.
  const fn code(&self) -> Option<AuthorityErrorCode> {
    match self {
      Self::None => None,
      Self::DecisionRequired { .. } => Some(AuthorityErrorCode::DecisionRequired),
      Self::UnsupportedContract { .. } => Some(AuthorityErrorCode::UnsupportedContract),
      Self::BundleMissing { .. } => Some(AuthorityErrorCode::BundleMissing),
    }
  }
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
    let requires = matches!(
      code,
      AuthorityErrorCode::DecisionRequired | AuthorityErrorCode::UnsupportedContract
    );
    let fits = match details.code() {
      None => !requires,
      Some(own) => own == code,
    };
    if !fits {
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

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DetailsWire {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  decisions: Option<Vec<DecisionId>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  required_version: Option<ReleaseVersion>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  reason: Option<BundleMissingReason>,
}

impl TryFrom<Wire> for AuthorityError {
  type Error = AuthorityErrorShapeError;

  fn try_from(wire: Wire) -> Result<Self, Self::Error> {
    let DetailsWire {
      decisions,
      required_version,
      reason,
    } = wire.details;
    let details = match (decisions, required_version, reason) {
      (None, None, None) => ErrorDetails::None,
      (Some(decisions), None, None) => ErrorDetails::DecisionRequired { decisions },
      (None, Some(required_version), None) => ErrorDetails::UnsupportedContract { required_version },
      (None, None, Some(reason)) => ErrorDetails::BundleMissing { reason },
      _ => return Err(AuthorityErrorShapeError::DetailsMismatch(wire.code)),
    };
    Self::new(wire.operation_id, wire.code, details)
  }
}

impl From<AuthorityError> for Wire {
  fn from(error: AuthorityError) -> Self {
    let details = match error.details {
      ErrorDetails::None => DetailsWire::default(),
      ErrorDetails::DecisionRequired { decisions } => DetailsWire {
        decisions: Some(decisions),
        ..DetailsWire::default()
      },
      ErrorDetails::UnsupportedContract { required_version } => DetailsWire {
        required_version: Some(required_version),
        ..DetailsWire::default()
      },
      ErrorDetails::BundleMissing { reason } => DetailsWire {
        reason: Some(reason),
        ..DetailsWire::default()
      },
    };
    Self {
      operation_id: error.operation_id,
      code: error.code,
      details,
    }
  }
}
