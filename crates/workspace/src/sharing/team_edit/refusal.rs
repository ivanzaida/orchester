//! What the team server refused a version with, as the hub says it in place (`Agents · v2` `acFkz`, A14 `gTLx6` no
//! longer an admin, A15 `dLqHC` the name is taken, A17 `rJbPm` the server rejects the content): the server's error code
//! as it was kept on the version ([`recorded`]), with the reason of a `422 invalid` after it, read back as one of the
//! few refusals a person can act on. Anything else is [`Refusal::Other`], whose code a message names.

use orchester_api::{ErrorCode, SharingErrorDetails};

use crate::messages::Message;

/// Why the team server did not take a created or published version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
  /// The person is no longer an admin of the team (`403 not_admin`): the draft stays here and only an admin publishes
  /// it.
  NotAdmin,
  /// Another resource of its kind has the name (`409 slug_taken`): nothing was created.
  NameTaken,
  /// The server checked the content and does not take it.
  Rejected(Rejection),
  /// Someone published a version from the same base first (`409 version_conflict`).
  Changed,
  /// Any other refusal, by its code.
  Other(String),
}

/// What the server rejects a version's content for (`422 invalid`'s reason, or `409 archived`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
  /// Its body breaks a rule of the team's form: a path, a value or a local id.
  Body,
  /// It names a team resource the server does not have or has archived, or a reference twice.
  References,
  /// It is not a version at all.
  Shape,
  /// The resource was archived on the server.
  Archived,
}

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
      _ => Self::Other(code.to_owned()),
    }
  }

  /// Why, in words, for a caller that has one place to say it.
  pub fn message(&self) -> Message {
    match self {
      Self::NotAdmin => Message::new("sharing.errors.notAdmin"),
      Self::NameTaken => Message::new("sharing.edit.refused.nameTaken"),
      Self::Rejected(rejection) => Message::new(match rejection {
        Rejection::Body => "sharing.edit.refused.body",
        Rejection::References => "sharing.edit.refused.references",
        Rejection::Shape => "sharing.edit.refused.shape",
        Rejection::Archived => "sharing.edit.refused.archived",
      }),
      Self::Changed => Message::new("sharing.edit.refused.changed"),
      Self::Other(code) => Message::new("sharing.edit.refused.other").with("code", code.clone()),
    }
  }
}

/// The code to keep on a version the server refused: the server's own, followed after a colon by the reason of a
/// `422 invalid`, which says what to fix.
pub(in crate::sharing) fn recorded(code: ErrorCode, details: Option<&SharingErrorDetails>) -> String {
  let quoted = serde_json::to_string(&code).expect("an error code serializes as a string");
  let code = quoted.trim_matches('"');
  match details {
    Some(SharingErrorDetails::Reason { reason }) if code == "invalid" => format!("{code}:{reason}"),
    _ => code.to_owned(),
  }
}
