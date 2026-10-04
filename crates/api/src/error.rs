use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
  BadRequest,
  PayloadTooLarge,
  MethodNotAllowed,
  RequestTimeout,
  InvalidCredentials,
  Unauthorized,
  PasswordChangeRequired,
  Forbidden,
  NotFound,
  Invalid,
  UsernameTaken,
  LastAdmin,
  NotMember,
  RemoteTaken,
  NoSuchUser,
  AlreadyMember,
  LastOwner,
  IdTaken,
  Conflict,
  RevisionConflict,
  NotEditable,
  NotQueueable,
  Organization,
  ClaimHeld,
  TaskMerged,
  NotHolder,
  ClaimNotStale,
  ClaimChanged,
  TagExists,
  SameTag,
  RateLimited,
  Internal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
  pub error: ErrorCode,
  /// English diagnostics; desktop text is translated by error code.
  pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SharingError {
  #[serde(flatten)]
  pub error: ApiError,
  #[serde(flatten)]
  pub details: SharingErrorDetails,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum SharingErrorDetails {
  Remote {
    project: orchester_core::id::ProjectId,
    member: bool,
    owners: Vec<crate::UserRef>,
  },
  Owners {
    owners: Vec<crate::UserRef>,
  },
  Revision {
    current: Box<crate::SharedTask>,
  },
  Claim {
    #[serde(deserialize_with = "required_claim")]
    claim: Option<crate::SharedClaim>,
  },
  Queue {
    missing: Vec<String>,
    reason: String,
  },
  Reason {
    reason: String,
  },
  Missing {
    missing: Vec<String>,
  },
}

// Option fields normally accept absence; a claim detail must explicitly contain claim, even when null.
fn required_claim<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Option<crate::SharedClaim>, D::Error> {
  Option::deserialize(de)
}
