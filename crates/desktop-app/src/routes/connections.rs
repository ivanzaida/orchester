use orchester_core::id::ConnectionId;

use super::{
  RouteError, TAB,
  query::{Params, with_query},
};

const SELECTED: &str = "selected";
const KIND: &str = "kind";

/// A tab of Connections (`Tabs` `qVM0O` in `z4QquH`): every connection, or one kind. All connections is the
/// one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ConnectionsTab {
  #[default]
  All,
  AgentProviders,
  GitHub,
  McpServers,
}

impl ConnectionsTab {
  pub const ALL: [Self; 4] = [Self::All, Self::AgentProviders, Self::GitHub, Self::McpServers];

  /// The tab's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::All => "all",
      Self::AgentProviders => "agent_providers",
      Self::GitHub => "github",
      Self::McpServers => "mcp_servers",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }
}

/// What Connections shows: its tab, and the connection whose inspector is open, if one is chosen; without
/// one the tab's first connection is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConnectionsView {
  tab: ConnectionsTab,
  selected: Option<ConnectionId>,
}

impl ConnectionsView {
  pub const fn new(tab: ConnectionsTab) -> Self {
    Self { tab, selected: None }
  }

  #[must_use]
  pub const fn with_selected(mut self, connection: ConnectionId) -> Self {
    self.selected = Some(connection);
    self
  }

  pub const fn tab(self) -> ConnectionsTab {
    self.tab
  }

  pub const fn selected(self) -> Option<ConnectionId> {
    self.selected
  }

  pub(super) fn path(self, connections: String) -> String {
    let tab = (self.tab != ConnectionsTab::default()).then(|| self.tab.name());
    let selected = self.selected.map(|id| id.to_string());
    with_query(connections, &[(TAB, tab), (SELECTED, selected.as_deref())])
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let mut view = Self::new(params.take_or_default(TAB, ConnectionsTab::from_name)?);
    if let Some(id) = params.take(SELECTED)? {
      view.selected = Some(id.parse().map_err(|_| RouteError::InvalidParam {
        param: SELECTED.to_owned(),
        value: id,
      })?);
    }
    Ok(view)
  }
}

/// What Add connection adds (`Add connection` menu in `oK3aE`): an agent provider (`cEJy4`), the one it
/// opens on, or an MCP server (`euVYY`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NewConnection {
  #[default]
  AgentProvider,
  McpServer,
}

impl NewConnection {
  pub const ALL: [Self; 2] = [Self::AgentProvider, Self::McpServer];

  /// The kind's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::AgentProvider => "agent_provider",
      Self::McpServer => "mcp_server",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|kind| kind.name() == name)
  }

  pub(super) fn path(self, new: String) -> String {
    let kind = (self != Self::default()).then(|| self.name());
    with_query(new, &[(KIND, kind)])
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    params.take_or_default(KIND, Self::from_name)
  }
}
