//! The project's Memory page's tabs (`Memory with tabs` `wMr98`), kept in its route
//! (`/project/<id>/memory?tab=waiting`). Without a tab the page opens on the one it chooses: *Waiting for approval*
//! while proposals wait, else *Active*. The Needs you panel's *Review records* opens
//! `ProjectPage::Memory(Some(MemoryTab::Waiting))`. `tab=settings`, the Settings tab the page had, opens Project
//! settings › Context, where its settings are now (`S15` `BJQn0`).

use super::{ProjectPage, ProjectSettingsTab, RouteError, TAB, query::Params};

/// The name of the Settings tab the Memory page had.
const MOVED_SETTINGS: &str = "settings";

/// A tab of the project's Memory page: one per status of its records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryTab {
  Waiting,
  Active,
  Deprecated,
  Superseded,
  /// The rejected proposals and the withdrawn ones.
  Rejected,
}

impl lurq::app::component::DevtoolsInspectable for MemoryTab {}

impl MemoryTab {
  pub const ALL: [Self; 5] = [
    Self::Waiting,
    Self::Active,
    Self::Deprecated,
    Self::Superseded,
    Self::Rejected,
  ];

  /// The tab's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Waiting => "waiting",
      Self::Active => "active",
      Self::Deprecated => "deprecated",
      Self::Superseded => "superseded",
      Self::Rejected => "rejected",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  /// The page a Memory route's tab `name` opens: the Memory page on that tab, or on the one it chooses when `name` is
  /// none; Project settings › Context for the Settings tab the page had. `None` when `name` names no tab.
  pub fn page(name: Option<&str>) -> Option<ProjectPage> {
    match name {
      Some(MOVED_SETTINGS) => Some(ProjectPage::Settings(ProjectSettingsTab::Context)),
      Some(name) => Self::from_name(name).map(|tab| ProjectPage::Memory(Some(tab))),
      None => Some(ProjectPage::Memory(None)),
    }
  }

  /// The query parameter of `tab`; none when the page chooses.
  pub(super) fn param(tab: Option<Self>) -> (&'static str, Option<&'static str>) {
    (TAB, tab.map(Self::name))
  }

  /// The page the query's tab names ([`Self::page`]).
  pub(super) fn take(params: &mut Params) -> Result<ProjectPage, RouteError> {
    let name = params.take(TAB)?;
    Self::page(name.as_deref()).ok_or_else(|| RouteError::InvalidParam {
      param: TAB.to_owned(),
      value: name.unwrap_or_default(),
    })
  }
}
