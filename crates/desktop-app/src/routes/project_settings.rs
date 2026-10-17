//! Project settings' tabs (`RExz4`), kept in its route (`/project/<id>/settings?tab=limits`).

use super::{RouteError, TAB, query::Params};

/// A tab of Project settings; General is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProjectSettingsTab {
  #[default]
  General,
  Limits,
  Environment,
  CodeIndex,
  Sharing,
}

impl ProjectSettingsTab {
  pub const ALL: [Self; 5] = [
    Self::General,
    Self::Limits,
    Self::Environment,
    Self::CodeIndex,
    Self::Sharing,
  ];

  /// The tab's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::General => "general",
      Self::Limits => "limits",
      Self::Environment => "environment",
      Self::CodeIndex => "code_index",
      Self::Sharing => "sharing",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  /// The query parameter of a tab other than the one Project settings opens on.
  pub(super) fn param(self) -> (&'static str, Option<&'static str>) {
    (TAB, (self != Self::default()).then(|| self.name()))
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    params.take_or_default(TAB, Self::from_name)
  }
}
