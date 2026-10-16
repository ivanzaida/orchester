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
  /// API 3: a cloud identity token that is malformed, forged, expired, used or for another server.
  InvalidIdentity,
  /// API 3: a valid identity with no account here and no open invite for it.
  NotInvited,
  /// API 3: the server does not accept cloud identities (not linked, or turned off).
  CloudOff,
  /// API 3: the server's copy of the cloud's keys expired; a newer key set is needed.
  KeySetStale,
  /// API 3: one answer for an unknown, used and expired code.
  InvalidCode,
  /// API 3: the server holds its maximum of open codes.
  TooManyCodes,
  /// API 3: another project has this slug, or the slug is reserved.
  SlugTaken,
  /// API 3: the join waited for approval and was refused.
  ApprovalRefused,
  /// API 3: a sign-in to the server admin by an account that is not an admin and owns no project.
  NothingToManage,
  /// API 3: join approval is on, so a sign-in to the server admin through Orchester Cloud is refused: sign in with a
  /// code made on one of the account's own computers (a pairing code), or an account code from the host.
  ApprovalRequired,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
