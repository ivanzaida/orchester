//! An admin's change of a team resource, sent straight to the team server (`docs/design/team-configuration-live.md`,
//! rule 6): Save publishes the next version on the latest version this session read of the resource, and Create makes
//! the resource with its first version. The server's answer is what memory holds next ([`crate::team_config`]);
//! nothing of the change is kept on this computer before or instead. Without the server there is nothing to save to,
//! so the change is refused with why ([`EditRefused::Unreachable`]). A version someone published first is refused by
//! the server: the resource's latest version is read again into memory, and the panel reloads from it
//! ([`EditRefused::Changed`]). A refusal is said in place ([`Refusal`]).

mod publication;
mod refusal;
mod send;

use orchester_api::ReferenceTarget;
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{ResourceKind, VersionNumber},
};
use serde_json::Value;

pub(super) use self::refusal::recorded;
pub use self::refusal::{Local, Refusal, Rejection};
use crate::messages::Message;

/// A version's body as the team holds it, and the references that body names. The files a skill's body names by hash
/// are uploaded from the project's blobs when the server asks for them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publication {
  pub body: Value,
  pub references: Vec<ReferenceTarget>,
}

/// A new team resource: the id this computer made for it, its kind, its name and its first version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTeamResource {
  pub id: ResourceId,
  pub kind: ResourceKind,
  pub name: Name,
  pub publication: Publication,
}

/// Why a change of a team resource was not saved; nothing changed on the team server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditRefused {
  /// The team server, or this computer, which knows the team cannot take it, refused it.
  Refused(Refusal),
  /// Someone published `latest` first. It is what this computer holds of the resource now, and what an edit starts
  /// from again.
  Changed { latest: VersionNumber },
  /// The team server cannot be reached, for the reason given: there is nothing to save to.
  Unreachable(Message),
  /// It could not be sent, or its answer could not be read; the words say why.
  Failed(Message),
}

impl EditRefused {
  /// What a press that only a person may make says when automation made it: nothing was sent.
  pub fn person_only() -> Self {
    Self::Failed(Self::person_only_message())
  }

  /// What a press that only a person may make says when automation made it, for a caller that has no [`EditRefused`].
  pub fn person_only_message() -> Message {
    Message::new("sharing.errors.personOnly")
  }

  /// Why, in words, for a caller that has one place to say it.
  pub fn message(&self) -> Message {
    match self {
      Self::Refused(refusal) => refusal.message(),
      Self::Changed { latest } => Refusal::Changed.message().with("version", latest.get().to_string()),
      Self::Unreachable(reason) | Self::Failed(reason) => reason.clone(),
    }
  }
}
