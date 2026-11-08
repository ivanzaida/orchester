//! What a version was refused with, as the hub says it in place (`Agents · v2` `acFkz`, A14 `gTLx6` no longer an admin,
//! A15 `dLqHC` the name is taken, A17 `rJbPm` the server rejects the content): the code as it was kept on the version
//! ([`recorded`]) read back as one of the few refusals a person can act on, either the team server's or this computer's
//! own, which found the team cannot take it and sent nothing. Anything else is [`Refusal::Other`], whose code a message
//! names.

use orchester_api::{ErrorCode, SharingErrorDetails};

use crate::messages::Message;

/// Why a created or published version was not taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
  /// The person is no longer an admin of the team (`403 not_admin`): the draft stays here and only an admin publishes
  /// it.
  NotAdmin,
  /// The same, found on this computer, which last read the person as no admin: nothing was sent.
  NotAdminHere,
  /// Another resource of its kind has the name (`409 slug_taken`): nothing was created.
  NameTaken,
  /// The server checked the content and does not take it.
  Rejected(Rejection),
  /// Someone published a version from the same base first (`409 version_conflict`).
  Changed,
  /// This computer found the team cannot take it, so nothing was sent.
  Local(Local),
  /// Any other refusal, by its code.
  Other(String),
}

/// What the team server rejects a version's content for (`422 invalid`'s reason, or `409 archived`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
  /// Its body breaks a rule of the team's form: a path, a value or a local id.
  Body,
  /// It names a team resource the server does not have or has archived, or a reference twice.
  References,
  /// It is not a version at all.
  Shape,
  /// It carries a list of agent tools or models the server cannot take.
  Catalog,
  /// The resource was archived on the server.
  Archived,
  /// Any other content the server says is invalid, without saying why.
  Unspecified,
}

/// What this computer finds the team cannot take, which it keeps on the version as refused instead of sending it again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Local {
  /// It names a connection, MCP server or secret of this computer.
  Entities,
  /// A pipeline runs a profile or tool that is not the team's.
  NamesNonTeam,
  /// A profile has writable folders, which are paths of this computer.
  WritableFolders,
}

/// The reasons of a `422 invalid` the hub says in words; any other is kept as no reason at all.
const REASONS: [&str; 4] = ["body", "references", "shape", "catalog"];

impl Refusal {
  /// The refusal a code kept on a version ([`recorded`]) stands for.
  pub fn of_code(code: &str) -> Self {
    match code.split_once(':').unwrap_or((code, "")) {
      ("not_admin", _) => Self::NotAdmin,
      ("slug_taken", _) => Self::NameTaken,
      ("version_conflict", _) => Self::Changed,
      ("archived", _) => Self::Rejected(Rejection::Archived),
      ("invalid", "body") => Self::Rejected(Rejection::Body),
      ("invalid", "references") => Self::Rejected(Rejection::References),
      ("invalid", "shape") => Self::Rejected(Rejection::Shape),
      ("invalid", "catalog") => Self::Rejected(Rejection::Catalog),
      ("invalid", _) => Self::Rejected(Rejection::Unspecified),
      ("local_entities_named", _) => Self::Local(Local::Entities),
      ("pipeline_names_non_team", _) => Self::Local(Local::NamesNonTeam),
      ("writable_folders_stay", _) => Self::Local(Local::WritableFolders),
      _ => Self::Other(code.to_owned()),
    }
  }

  /// Whether this computer found it, not the team's server: nothing reached the server.
  pub const fn is_local(&self) -> bool {
    matches!(self, Self::NotAdminHere | Self::Local(_))
  }

  /// Why, in words, for a caller that has one place to say it.
  pub fn message(&self) -> Message {
    match self {
      Self::NotAdmin => Message::new("sharing.errors.notAdmin"),
      Self::NotAdminHere => Message::new("sharing.edit.refused.notAdminHere"),
      Self::NameTaken => Message::new("sharing.edit.refused.nameTaken"),
      Self::Rejected(rejection) => Message::new(match rejection {
        Rejection::Body => "sharing.edit.refused.body",
        Rejection::References => "sharing.edit.refused.references",
        Rejection::Shape => "sharing.edit.refused.shape",
        Rejection::Catalog => "sharing.edit.refused.catalog",
        Rejection::Archived => "sharing.edit.refused.archived",
        Rejection::Unspecified => "sharing.edit.refused.unspecified",
      }),
      Self::Changed => Message::new("sharing.edit.refused.changed"),
      Self::Local(local) => Message::new(match local {
        Local::Entities => "sharing.edit.refused.localEntities",
        Local::NamesNonTeam => "sharing.edit.refused.pipelineNonTeam",
        Local::WritableFolders => "sharing.edit.refused.writableFolders",
      }),
      Self::Other(code) => Message::new("sharing.edit.refused.other").with("code", code.clone()),
    }
  }
}

/// The code to keep on a version the server refused: the server's own, followed after a colon by the reason of a
/// `422 invalid` when it is one the hub says in words (`body`, `references`, `shape`, `catalog`), which says what to
/// fix.
pub(in crate::sharing) fn recorded(code: ErrorCode, details: Option<&SharingErrorDetails>) -> String {
  let quoted = serde_json::to_string(&code).expect("an error code serializes as a string");
  let code = quoted.trim_matches('"');
  match details {
    Some(SharingErrorDetails::Reason { reason }) if code == "invalid" && REASONS.contains(&reason.as_str()) => {
      format!("{code}:{reason}")
    }
    _ => code.to_owned(),
  }
}
