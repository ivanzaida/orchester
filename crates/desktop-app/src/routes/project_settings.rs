//! Project settings' own tabs (`Project settings · redesign` `XGtkg`), kept in its route
//! (`/project/<id>/settings?tab=runs`). Connections and Secrets are tabs of the page too, with routes of their own
//! (`/project/<id>/connections`, `/project/<id>/secrets`).

use super::{RouteError, TAB, query::Params};

/// A tab of Project settings with no route of its own; General is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProjectSettingsTab {
  #[default]
  General,
  /// Auto mode, the limits and the environment of new runs.
  Runs,
  /// The sources agents may read without asking (`kmNiF`) and the code index.
  Context,
  Sharing,
}

impl ProjectSettingsTab {
  pub const ALL: [Self; 4] = [Self::General, Self::Runs, Self::Context, Self::Sharing];

  /// The tab's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::General => "general",
      Self::Runs => "runs",
      Self::Context => "context",
      Self::Sharing => "sharing",
    }
  }

  /// The tab `name` names; the names of the tabs Runs and Context took in (`limits`, `environment`, `sources`,
  /// `code_index`) still lead to them, so a path kept from before they moved opens where its setting is now.
  pub fn from_name(name: &str) -> Option<Self> {
    match name {
      "limits" | "environment" => Some(Self::Runs),
      "sources" | "code_index" => Some(Self::Context),
      name => Self::ALL.into_iter().find(|tab| tab.name() == name),
    }
  }

  /// The query parameter of a tab other than the one Project settings opens on.
  pub(super) fn param(self) -> (&'static str, Option<&'static str>) {
    (TAB, (self != Self::default()).then(|| self.name()))
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    params.take_or_default(TAB, Self::from_name)
  }
}
