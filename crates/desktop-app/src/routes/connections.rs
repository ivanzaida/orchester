use orchester_core::id::ConnectionId;

use super::{
  RouteError,
  query::{Params, with_query},
};

const SELECTED: &str = "selected";
const KIND: &str = "kind";

/// What Project settings › Connections shows (`05 Connections` `dk46o`): the project's connections in their groups,
/// or one connection on its own page (`05b` `PGwka`, `13` `Ptkxo`, `14` `KItiZ`) when one is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConnectionsView {
  selected: Option<ConnectionId>,
}

impl ConnectionsView {
  /// The page of `connection`.
  pub const fn of(connection: ConnectionId) -> Self {
    Self {
      selected: Some(connection),
    }
  }

  pub const fn selected(self) -> Option<ConnectionId> {
    self.selected
  }

  pub(super) fn path(self, connections: String) -> String {
    let selected = self.selected.map(|id| id.to_string());
    with_query(connections, &[(SELECTED, selected.as_deref())])
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let Some(id) = params.take(SELECTED)? else {
      return Ok(Self::default());
    };
    id.parse().map(Self::of).map_err(|_| RouteError::InvalidParam {
      param: SELECTED.to_owned(),
      value: id,
    })
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
