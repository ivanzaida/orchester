//! What fixes a part this computer lacks, which a row of *Ready here?* offers at its end and the pinned action offers
//! for the first missing part (`Link` `rfGmw`, `Actions` `o8tFHR` in `Agents · Profile` `NDZXC`).

use orchester_core::{
  id::ResourceId,
  secret::SecretName,
  sharing::{ConnectionProblem, Missing},
};

use crate::ui::icon::Icon;

/// A fix this computer takes by itself; a part this panel does not fix (a server this app cannot run, a keychain
/// that cannot say, a reviewer's connection) has none, and no action stands in its place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Fix {
  /// Keep the connection the person chose among several as the profile's.
  UseConnection,
  /// Go where a connection of the CLI kind is added.
  AddConnection,
  SwitchOn(ResourceId),
  SetPath(ResourceId),
  AddValue(SecretName),
}

impl Fix {
  pub(super) fn of(part: &Missing) -> Option<Self> {
    match part {
      Missing::Connection {
        problem: ConnectionProblem::PickNeeded,
        ..
      } => Some(Self::UseConnection),
      Missing::Connection {
        problem: ConnectionProblem::NoneOfKind,
        ..
      } => Some(Self::AddConnection),
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
  pub(super) const fn link_key(&self) -> &'static str {
    match self {
      Self::UseConnection => "team_hub.fix.link.use",
      Self::AddConnection => "team_hub.fix.link.add",
      Self::SwitchOn(_) => "team_hub.fix.link.switchOn",
      Self::SetPath(_) => "team_hub.fix.link.setPath",
      Self::AddValue(_) => "team_hub.fix.link.addValue",
    }
  }

  /// The glyph of the pinned action.
  pub(super) const fn glyph(&self) -> Icon {
    match self {
      Self::UseConnection => Icon::Check,
      Self::AddConnection => Icon::Plus,
      Self::SwitchOn(_) => Icon::Power,
      Self::SetPath(_) => Icon::FolderOpen,
      Self::AddValue(_) => Icon::KeyRound,
    }
  }

  /// What the id of the action's control says it is.
  pub(super) const fn name(&self) -> &'static str {
    match self {
      Self::UseConnection => "use",
      Self::AddConnection => "add",
      Self::SwitchOn(_) => "switch-on",
      Self::SetPath(_) => "set-path",
      Self::AddValue(_) => "add-value",
    }
  }
}
