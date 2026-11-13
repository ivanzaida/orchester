//! The Pipelines hub (`Pipelines · v2` `pCzIS`, P1): the tab it filters by and the pipeline its
//! panel shows, kept in its route (`/project/<id>/pipelines?filter=archived&item=<id>`).

use orchester_core::id::ResourceId;

use super::{RouteError, query::Params};

const FILTER: &str = "filter";
const ITEM: &str = "item";

/// Which pipelines the list shows; All is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PipelinesFilter {
  #[default]
  All,
  Archived,
}

/// The name of the *Drafts* tab, gone with pipeline drafts (D-032), which a route kept from before (the history, a
/// link) may still name.
const GONE_DRAFTS: &str = "drafts";

impl PipelinesFilter {
  pub const ALL: [Self; 2] = [Self::All, Self::Archived];

  /// The filter's name in a path's query and in the tab's element id.
  pub const fn name(self) -> &'static str {
    match self {
      Self::All => "all",
      Self::Archived => "archived",
    }
  }

  /// The filter named `name`, as [`Self::name`] spells it; the gone *Drafts* tab's name opens *All*, which lists
  /// every pipeline it did.
  pub fn from_name(name: &str) -> Option<Self> {
    if name == GONE_DRAFTS {
      return Some(Self::All);
    }
    Self::ALL.into_iter().find(|filter| filter.name() == name)
  }
}

/// The hub's tab and the pipeline its panel shows; `None` for the first one the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PipelinesView {
  pub filter: PipelinesFilter,
  pub item: Option<ResourceId>,
}

impl PipelinesView {
  pub const fn new(filter: PipelinesFilter, item: Option<ResourceId>) -> Self {
    Self { filter, item }
  }

  /// `path` with the tab and the pipeline the hub is not opened on.
  pub(super) fn path(self, path: String) -> String {
    let item = self.item.map(|item| item.to_string());
    let filter = (self.filter != PipelinesFilter::default()).then(|| self.filter.name());
    super::with_query(path, &[(FILTER, filter), (ITEM, item.as_deref())])
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    Ok(Self {
      filter: params.take_or_default(FILTER, PipelinesFilter::from_name)?,
      item: params.take_parsed(ITEM)?,
    })
  }
}
