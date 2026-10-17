//! The Skills library's filter tabs (`Syllb`), kept in its route (`/project/<id>/skills?filter=roles`).

use super::{RouteError, query::Params};

const FILTER: &str = "filter";

/// Which skills the library lists: every one, the one it opens on, or those of one scope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SkillsFilter {
  #[default]
  All,
  Project,
  Roles,
  Pipelines,
}

impl SkillsFilter {
  pub const ALL: [Self; 4] = [Self::All, Self::Project, Self::Roles, Self::Pipelines];

  pub const fn name(self) -> &'static str {
    match self {
      Self::All => "all",
      Self::Project => "project",
      Self::Roles => "roles",
      Self::Pipelines => "pipelines",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|filter| filter.name() == name)
  }

  /// The query parameter of a filter other than the one the library opens on.
  pub(super) fn param(self) -> (&'static str, Option<&'static str>) {
    (FILTER, (self != Self::default()).then(|| self.name()))
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    params.take_or_default(FILTER, Self::from_name)
  }
}
