//! The project's Memory page's tabs (`Memory with tabs` `wMr98`), kept in its route
//! (`/project/<id>/memory?tab=waiting`). Without a tab the page opens on the one it chooses: *Waiting for approval*
//! while proposals wait, else *Active*. The Needs you panel's *Review records* opens
//! `ProjectPage::Memory(Some(MemoryTab::Waiting))`.

use super::{RouteError, TAB, query::Params};

/// A tab of the project's Memory page: one per status of its records, then Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryTab {
  Waiting,
  Active,
  Deprecated,
  Superseded,
  /// The rejected proposals and the withdrawn ones.
  Rejected,
  Settings,
}

impl MemoryTab {
  pub const ALL: [Self; 6] = [
    Self::Waiting,
    Self::Active,
    Self::Deprecated,
    Self::Superseded,
    Self::Rejected,
    Self::Settings,
  ];

  /// The tab's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Waiting => "waiting",
      Self::Active => "active",
      Self::Deprecated => "deprecated",
      Self::Superseded => "superseded",
      Self::Rejected => "rejected",
      Self::Settings => "settings",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  /// The query parameter of `tab`; none when the page chooses.
  pub(super) fn param(tab: Option<Self>) -> (&'static str, Option<&'static str>) {
    (TAB, tab.map(Self::name))
  }

  pub(super) fn take(params: &mut Params) -> Result<Option<Self>, RouteError> {
    params.take_or_default(TAB, |name| Self::from_name(name).map(Some))
  }
}
