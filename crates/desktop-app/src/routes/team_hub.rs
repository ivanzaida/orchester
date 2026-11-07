//! The Agents hub of a team project (`Agents · v2` `acFkz`): the kind it filters by and the resource its panel
//! shows, kept in its route (`/project/<id>/agents?filter=mcp_servers&item=<id>`).

use orchester_core::id::ResourceId;

use super::{RouteError, query::Params};

const FILTER: &str = "filter";
const ITEM: &str = "item";

/// Which kinds the hub's list shows; All is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HubFilter {
  #[default]
  All,
  Profiles,
  McpServers,
  Instructions,
  Skills,
  Tools,
}

impl HubFilter {
  pub const ALL: [Self; 6] = [
    Self::All,
    Self::Profiles,
    Self::McpServers,
    Self::Instructions,
    Self::Skills,
    Self::Tools,
  ];

  /// The filter's name in a path's query and in the filter's element ids.
  pub const fn name(self) -> &'static str {
    match self {
      Self::All => "all",
      Self::Profiles => "profiles",
      Self::McpServers => "mcp_servers",
      Self::Instructions => "instructions",
      Self::Skills => "skills",
      Self::Tools => "tools",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|filter| filter.name() == name)
  }
}

/// The hub's filter and the resource its panel shows; `None` for the first one the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HubView {
  pub filter: HubFilter,
  pub item: Option<ResourceId>,
}

impl HubView {
  pub const fn new(filter: HubFilter, item: Option<ResourceId>) -> Self {
    Self { filter, item }
  }

  /// `path` with the filter and the item the hub is not opened on.
  pub(super) fn path(self, path: String) -> String {
    let item = self.item.map(|item| item.to_string());
    let filter = (self.filter != HubFilter::default()).then(|| self.filter.name());
    super::with_query(path, &[(FILTER, filter), (ITEM, item.as_deref())])
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let filter = params.take_or_default(FILTER, HubFilter::from_name)?;
    let item = params.take_parsed(ITEM)?;
    Ok(Self { filter, item })
  }
}
