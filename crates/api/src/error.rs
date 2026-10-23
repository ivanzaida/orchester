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
  /// Another module of the project has this name, without regard to case; the error names it
  /// ([`SharingErrorDetails::Module`]).
  ModuleExists,
  /// Core's module rules refused a module or a task's module (`reason`: `target_before_start`, `module_not_found`).
  Module,
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
  /// API 3: sending an invite by email needs a browser that signed in to the admin through Orchester Cloud with
  /// two-factor authentication; an invite's join token must say so too.
  MfaRequired,
  /// API 3: entering a code in the app needs a cloud identity with two-factor authentication, which this server cannot
  /// check: it has no Orchester Cloud root key pinned (`orchester-server cloud pin`, or linking it).
  MfaUnverifiable,
  /// API 3: a code that makes an account, entered with a cloud identity that an account here is linked to already:
  /// that person has an account on the server, and joins with it rather than with a second one.
  AccountExists,
  /// API 3: an invite by email to an address that has an open one to the same project (or to the server) already.
  /// The error names that invite and when it was sent ([`SharingErrorDetails::Invite`]); revoking it lets a new one go.
  InviteOpen,
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
  /// A module's `409 revision_conflict`: the module as it is now.
  ModuleRevision {
    current: Box<crate::SharedModule>,
  },
  /// `409 module_exists`: the module that has the name.
  Module {
    module: Box<crate::SharedModule>,
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
  /// The open invite an invite by email would repeat; `sent_at` in milliseconds since the Unix epoch.
  Invite {
    invite: orchester_core::id::InviteId,
    sent_at: i64,
  },
}

// Option fields normally accept absence; a claim detail must explicitly contain claim, even when null.
fn required_claim<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Option<crate::SharedClaim>, D::Error> {
  Option::deserialize(de)
}
