//! An agent profile editor's tabs (`gTrvs`), kept in its route (`/project/<id>/agents/<id>?tab=permissions`,
//! `/project/<id>/agents/new`).

use super::{RouteError, TAB, query::Params};

/// A tab of the profile editor; Profile is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AgentTab {
  #[default]
  Profile,
  Permissions,
  UsedBy,
}

impl AgentTab {
  pub const ALL: [Self; 3] = [Self::Profile, Self::Permissions, Self::UsedBy];

  /// The tab's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Profile => "profile",
      Self::Permissions => "permissions",
      Self::UsedBy => "used_by",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  /// The query parameter of a tab other than the one the editor opens on.
  pub(super) fn param(self) -> (&'static str, Option<&'static str>) {
    (TAB, (self != Self::default()).then(|| self.name()))
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    params.take_or_default(TAB, Self::from_name)
  }
}
