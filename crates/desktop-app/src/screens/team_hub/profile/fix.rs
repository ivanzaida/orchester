//! What fixes a part this computer lacks, which a row of *Ready here?* offers at its end and the pinned action offers
//! for the first missing part (`Link` `rfGmw`, `Actions` `o8tFHR` in `Agents · Profile` `NDZXC`).

use orchester_core::{
  id::ResourceId,
  secret::SecretName,
  sharing::{ConnectionProblem, Missing},
};
use orchester_workspace::readiness::{ReadinessStanding, Undecided};

use crate::{screens::team_hub::model::Entry, ui::icon::Icon};

/// A fix this computer takes by itself; a part this panel does not fix (a server this app cannot run, a keychain
/// that cannot say, a reviewer's connection) has none, and no action stands in its place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens) enum Fix {
  /// Keep the connection the person chose among several as the profile's.
  UseConnection,
  /// Go where a connection of the CLI kind is added.
  AddConnection,
  /// Go where the connection the profile runs on is signed in.
  SignIn,
  SwitchOn(ResourceId),
  SetPath(ResourceId),
  AddValue(SecretName),
  /// Open what the profile names that the team archived in the hub's *Archived*, where an admin restores it (A37
  /// `AFbIb`).
  OpenArchived(ResourceId),
}

impl Fix {
  pub(in crate::screens) fn of(part: &Missing) -> Option<Self> {
    match part {
      Missing::Connection {
        problem: ConnectionProblem::PickNeeded,
        ..
      } => Some(Self::UseConnection),
      Missing::Connection {
        problem: ConnectionProblem::NoneOfKind,
        ..
      } => Some(Self::AddConnection),
      Missing::ConnectionSignIn { .. } => Some(Self::SignIn),
      Missing::McpOff { server } => Some(Self::SwitchOn(*server)),
      Missing::McpProgram { server, .. } => Some(Self::SetPath(*server)),
      Missing::Secret { name } => Some(Self::AddValue(name.clone())),
      Missing::DecisionConnection { .. }
      | Missing::McpUnsupported { .. }
      | Missing::SecretUnreadable { .. }
      | Missing::LocalMcpProgram { .. }
      | Missing::LocalSecret { .. } => None,
    }
  }

  /// The words at a row's end.
  pub(in crate::screens) const fn link_key(&self) -> &'static str {
    match self {
      Self::UseConnection => "team_hub.fix.link.use",
      Self::AddConnection => "team_hub.fix.link.add",
      Self::SignIn => "team_hub.fix.link.signIn",
      Self::SwitchOn(_) => "team_hub.fix.link.switchOn",
      Self::SetPath(_) => "team_hub.fix.link.setPath",
      Self::AddValue(_) => "team_hub.fix.link.addValue",
      Self::OpenArchived(_) => "team_hub.fix.link.openArchived",
    }
  }

  /// The glyph of the pinned action.
  pub(in crate::screens) const fn glyph(&self) -> Icon {
    match self {
      Self::UseConnection => Icon::Check,
      Self::AddConnection => Icon::Plus,
      Self::SignIn => Icon::LogIn,
      Self::SwitchOn(_) => Icon::Power,
      Self::SetPath(_) => Icon::FolderOpen,
      Self::AddValue(_) => Icon::KeyRound,
      Self::OpenArchived(_) => Icon::Archive,
    }
  }

  /// What the id of the action's control says it is.
  pub(in crate::screens) const fn name(&self) -> &'static str {
    match self {
      Self::UseConnection => "use",
      Self::AddConnection => "add",
      Self::SignIn => "sign-in",
      Self::SwitchOn(_) => "switch-on",
      Self::SetPath(_) => "set-path",
      Self::AddValue(_) => "add-value",
      Self::OpenArchived(_) => "open-archived",
    }
  }
}

/// What the profile `entry` names that the team archived, which keeps it from running on any computer: its id and its
/// name (`docs/design/team-configuration.md`, edge case 8).
pub(in crate::screens::team_hub) fn archived_named(entry: &Entry) -> Option<(ResourceId, &str)> {
  match entry.standing.as_ref()? {
    ReadinessStanding::Undecided(
      Undecided::ChoosesArchivedMcpServer(id, name)
      | Undecided::UsesArchivedTool(id, name)
      | Undecided::UsesArchivedInstruction(id, name),
    ) => Some((*id, name.as_str())),
    _ => None,
  }
}

/// The fix to offer first for `entry`: opening what it names that the team archived, else the fix of the first part
/// its readiness misses.
pub(in crate::screens::team_hub) fn first(entry: &Entry) -> Option<Fix> {
  archived_named(entry).map_or_else(
    || {
      entry
        .standing
        .as_ref()
        .and_then(|standing| standing.missing().first())
        .and_then(Fix::of)
    },
    |(id, _)| Some(Fix::OpenArchived(id)),
  )
}

/// What a row of *Ready here?* shows of a fix: the words of its link and what its control is called in element ids. A
/// team project's [`Fix`] and a local project's offer it alike, so one section draws both.
pub(in crate::screens) trait Offer {
  fn link_key(&self) -> &'static str;
  fn name(&self) -> &'static str;
}

impl Offer for Fix {
  fn link_key(&self) -> &'static str {
    Self::link_key(self)
  }

  fn name(&self) -> &'static str {
    Self::name(self)
  }
}
