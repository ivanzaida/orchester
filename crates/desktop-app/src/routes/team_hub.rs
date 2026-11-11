//! The Agents hub of a team project (`Agents · v2` `acFkz`): the kind it filters by and the resource its panel
//! shows, kept in its route (`/project/<id>/agents?filter=mcp_servers&item=<id>`), or the form that creates a resource
//! of the filter's kind (`…?filter=mcp_servers&new=1`, A7–A12). Its Memory filter keeps its own route
//! ([`super::ProjectPage::Memory`]), which [`super::ProjectPage::hub`] gives a view on it.

use orchester_core::id::{ConnectionId, ResourceId};

use super::{MemoryView, RouteError, query::Params};

const FILTER: &str = "filter";
const ITEM: &str = "item";
const SERVER: &str = "server";
const NEW: &str = "new";

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
  /// What the project's agents remember on this computer (`Section Y · Memory` `Xxl1u`): a team project's too, never
  /// the team's.
  Memory,
  /// What a local project archived (`Agents · Archived` L13 `MXma9`); only a local project's hub offers it.
  Archived,
}

impl HubFilter {
  /// The filters a team project's hub offers.
  pub const ALL: [Self; 7] = [
    Self::All,
    Self::Profiles,
    Self::McpServers,
    Self::Instructions,
    Self::Skills,
    Self::Tools,
    Self::Memory,
  ];

  /// The filters a local project's hub offers: the team's and *Archived*.
  pub const LOCAL: [Self; 8] = [
    Self::All,
    Self::Profiles,
    Self::McpServers,
    Self::Instructions,
    Self::Skills,
    Self::Tools,
    Self::Memory,
    Self::Archived,
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
      Self::Memory => "memory",
      Self::Archived => "archived",
    }
  }

  /// The filter named `name`, as [`Self::name`] spells it.
  pub fn from_name(name: &str) -> Option<Self> {
    Self::LOCAL.into_iter().find(|filter| filter.name() == name)
  }
}

/// The hub's filter and the resource its panel shows; `None` for the first one the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HubView {
  pub filter: HubFilter,
  pub item: Option<ResourceId>,
  /// The local MCP server the panel shows: a local server is a connection with its own id, not a resource
  /// (`docs/design/team-configuration.md` rule 12); only a local project's hub has one.
  pub server: Option<ConnectionId>,
  /// The panel is the form that creates a resource of the filter's kind, which only a kind has (not All).
  pub adding: bool,
  /// The Memory filter's status and record.
  pub memory: MemoryView,
}

impl HubView {
  pub const fn new(filter: HubFilter, item: Option<ResourceId>) -> Self {
    Self {
      filter,
      item,
      server: None,
      adding: false,
      memory: MemoryView::new(None, None),
    }
  }

  /// The hub's Memory filter on `memory`.
  pub const fn memory(memory: MemoryView) -> Self {
    Self {
      memory,
      ..Self::new(HubFilter::Memory, None)
    }
  }

  /// The hub on the local MCP server `server`, under `filter`.
  pub const fn on_server(filter: HubFilter, server: ConnectionId) -> Self {
    Self {
      filter,
      item: None,
      server: Some(server),
      adding: false,
      memory: MemoryView::new(None, None),
    }
  }

  /// The form that creates a resource of the kind `filter` holds.
  pub const fn adding(filter: HubFilter) -> Self {
    Self {
      filter,
      item: None,
      server: None,
      adding: true,
      memory: MemoryView::new(None, None),
    }
  }

  /// `path` with the filter and the item the hub is not opened on.
  pub(super) fn path(self, path: String) -> String {
    let item = self.item.map(|item| item.to_string());
    let server = self.server.map(|server| server.to_string());
    let filter = (self.filter != HubFilter::default()).then(|| self.filter.name());
    let adding = self.adding.then_some("1");
    super::with_query(
      path,
      &[
        (FILTER, filter),
        (ITEM, item.as_deref()),
        (SERVER, server.as_deref()),
        (NEW, adding),
      ],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let filter = params.take_or_default(FILTER, HubFilter::from_name)?;
    let item = params.take_parsed(ITEM)?;
    let server = params.take_parsed(SERVER)?;
    // A form has no resource, and only a kind has a form; Memory's *New record* is its panel's.
    let adding =
      params.take(NEW)?.is_some_and(|new| new == "1") && !matches!(filter, HubFilter::All | HubFilter::Memory);
    Ok(Self {
      filter,
      item: item.filter(|_| !adding),
      server: server.filter(|_| !adding),
      adding,
      memory: MemoryView::default(),
    })
  }
}
