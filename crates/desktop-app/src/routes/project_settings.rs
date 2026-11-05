//! Project settings' own tabs (`Project settings · redesign` `XGtkg`), kept in its route
//! (`/project/<id>/settings?tab=runs`). Connections and Environment are tabs of the page too, with routes of their own
//! (`/project/<id>/connections`, and Environment the secrets' `/project/<id>/secrets`, the tab it was).

use super::{RouteError, TAB, query::Params};

/// A tab of Project settings with no route of its own; General is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProjectSettingsTab {
  #[default]
  General,
  /// Auto mode, the estimates and the limits of new runs.
  Runs,
  /// What the project's Orchestrator may do on its own (`S03` `X8mFTS`).
  Orchestrator,
  /// The sources agents may read without asking (`kmNiF`) and the code index.
  Context,
  Sharing,
}

impl ProjectSettingsTab {
  pub const ALL: [Self; 5] = [
    Self::General,
    Self::Runs,
    Self::Orchestrator,
    Self::Context,
    Self::Sharing,
  ];

  /// The tab's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::General => "general",
      Self::Runs => "runs",
      Self::Orchestrator => "orchestrator",
      Self::Context => "context",
      Self::Sharing => "sharing",
    }
  }

  /// The tab `name` names; the names of the tabs Runs and Context took in (`limits`, `sources`, `code_index`) still
  /// lead to them, so a path kept from before they moved opens where its setting is now.
  pub fn from_name(name: &str) -> Option<Self> {
    match name {
      "limits" => Some(Self::Runs),
      "sources" | "code_index" => Some(Self::Context),
      name => Self::ALL.into_iter().find(|tab| tab.name() == name),
    }
  }

  /// Whether `name` names the Environment tab (`S06` `EnL2D`), whose route is the secrets' own: its name, or
  /// `secrets`, the tab it was.
  pub fn is_environment(name: &str) -> bool {
    matches!(name, "environment" | "secrets")
  }

  /// The query parameter of a tab other than the one Project settings opens on.
  pub(super) fn param(self) -> (&'static str, Option<&'static str>) {
    (TAB, (self != Self::default()).then(|| self.name()))
  }

  /// The tab the query names, General when it names none; `None` when it names Environment.
  pub(super) fn take(params: &mut Params) -> Result<Option<Self>, RouteError> {
    let Some(name) = params.take(TAB)? else {
      return Ok(Some(Self::default()));
    };
    if Self::is_environment(&name) {
      return Ok(None);
    }
    match Self::from_name(&name) {
      Some(tab) => Ok(Some(tab)),
      None => Err(RouteError::InvalidParam {
        param: TAB.to_owned(),
        value: name,
      }),
    }
  }
}
