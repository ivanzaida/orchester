//! Import resources' kind tabs (`Kind tabs` in `DsqQJ`), kept in its route's query: each entry point opens it on
//! its own kind (`G69AHh`), and a tab a person picks is a place to go back to.

use orchester_core::resource::ResourceKind;

use super::{RouteError, TAB, query::Params};

/// A kind tab of Import resources; All is the one it opens on. Secrets are not imported: their tab says why.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImportTab {
  #[default]
  All,
  Pipelines,
  Agents,
  Instructions,
  Tools,
  Secrets,
}

impl ImportTab {
  pub const ALL: [Self; 6] = [
    Self::All,
    Self::Pipelines,
    Self::Agents,
    Self::Instructions,
    Self::Tools,
    Self::Secrets,
  ];

  pub const fn name(self) -> &'static str {
    match self {
      Self::All => "all",
      Self::Pipelines => "pipelines",
      Self::Agents => "agents",
      Self::Instructions => "instructions",
      Self::Tools => "tools",
      Self::Secrets => "secrets",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  /// The kind of resource the tab lists; `None` for All, and for Secrets, which lists none.
  pub const fn kind(self) -> Option<ResourceKind> {
    match self {
      Self::All | Self::Secrets => None,
      Self::Pipelines => Some(ResourceKind::Pipeline),
      Self::Agents => Some(ResourceKind::AgentProfile),
      Self::Instructions => Some(ResourceKind::Instruction),
      Self::Tools => Some(ResourceKind::Tool),
    }
  }

  /// The query naming the tab; none for All.
  pub(super) fn param(self) -> (&'static str, Option<&'static str>) {
    (TAB, (self != Self::All).then(|| self.name()))
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    params.take_or_default(TAB, Self::from_name)
  }
}
