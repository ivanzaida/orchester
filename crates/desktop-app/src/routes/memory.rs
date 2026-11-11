//! The Agents hub's Memory filter (`Agents · v2` `acFkz`, Section Y `Xxl1u`): the status its sub-filter shows and the
//! record its panel shows, kept in its route (`/project/<id>/memory?tab=waiting&record=<id>`). Without a status the
//! filter opens on the one it chooses: *Waiting for approval* while proposals wait, else *Active*; without a record,
//! on the first one the list shows. The Needs you panel's *Review records* opens it on *Waiting for approval*.
//! `tab=settings`, the Settings tab the Memory page had, opens Project settings › Context, where its settings are now
//! (`S15` `BJQn0`).

use orchester_core::id::MemoryId;

use super::{ProjectPage, ProjectSettingsTab, RouteError, TAB, query::Params};

/// The name of the Settings tab the Memory page had.
const MOVED_SETTINGS: &str = "settings";
const RECORD: &str = "record";

/// A status of the Memory filter's records, each a choice of its sub-filter.
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

  /// The status's name in a path's query and in the app's MCP tools.
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

  /// The page a Memory route's status `name` and `record` open: the Memory filter on that status, or on the one it
  /// chooses when `name` is none; Project settings › Context for the Settings tab the page had. `None` when `name`
  /// names no status.
  pub fn page(name: Option<&str>, record: Option<MemoryId>) -> Option<ProjectPage> {
    match name {
      Some(MOVED_SETTINGS) => Some(ProjectPage::Settings(ProjectSettingsTab::Context)),
      Some(name) => Self::from_name(name).map(|tab| ProjectPage::Memory(MemoryView::new(Some(tab), record))),
      None => Some(ProjectPage::Memory(MemoryView::new(None, record))),
    }
  }
}

/// The Memory filter's status and the record its panel shows; `None` for the status it opens on and the first record
/// of the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MemoryView {
  pub tab: Option<MemoryTab>,
  pub record: Option<MemoryId>,
}

impl lurq::app::component::DevtoolsInspectable for MemoryView {}

impl MemoryView {
  pub const fn new(tab: Option<MemoryTab>, record: Option<MemoryId>) -> Self {
    Self { tab, record }
  }

  /// The filter on `tab`, with the first record of its list.
  pub const fn on(tab: MemoryTab) -> Self {
    Self::new(Some(tab), None)
  }

  /// `path` with the status and the record the view names.
  pub(super) fn path(self, path: String) -> String {
    let record = self.record.map(|record| record.to_string());
    super::with_query(
      path,
      &[(TAB, self.tab.map(MemoryTab::name)), (RECORD, record.as_deref())],
    )
  }

  /// The page the query's status and record name ([`MemoryTab::page`]).
  pub(super) fn take(params: &mut Params) -> Result<ProjectPage, RouteError> {
    let name = params.take(TAB)?;
    let record = params.take_parsed(RECORD)?;
    MemoryTab::page(name.as_deref(), record).ok_or_else(|| RouteError::InvalidParam {
      param: TAB.to_owned(),
      value: name.unwrap_or_default(),
    })
  }
}
