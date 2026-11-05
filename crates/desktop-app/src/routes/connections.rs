use orchester_core::id::{ConnectionId, DecisionConnectionId};

use super::{
  RouteError,
  query::{Params, with_query},
};

const SELECTED: &str = "selected";
const KIND: &str = "kind";
const DECISION: &str = "decision";

/// What Project settings › Connections shows (`05 Connections` `dk46o`): the project's connections in their groups,
/// or one connection on its own page (`05b` `PGwka`, `13` `Ptkxo`, `14` `KItiZ`) when one is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConnectionsView {
  selected: Option<Selected>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Selected {
  Connection(ConnectionId),
  Decision(DecisionConnectionId),
}

impl ConnectionsView {
  /// The page of `connection`.
  pub const fn of(connection: ConnectionId) -> Self {
    Self {
      selected: Some(Selected::Connection(connection)),
    }
  }

  /// The page of a decision connection, whose id cannot select an agent connection.
  pub const fn decision(connection: DecisionConnectionId) -> Self {
    Self {
      selected: Some(Selected::Decision(connection)),
    }
  }

  pub const fn selected_decision(self) -> Option<DecisionConnectionId> {
    match self.selected {
      Some(Selected::Decision(id)) => Some(id),
      _ => None,
    }
  }

  pub const fn is_root(self) -> bool {
    self.selected.is_none()
  }

  pub const fn selected(self) -> Option<ConnectionId> {
    match self.selected {
      Some(Selected::Connection(id)) => Some(id),
      _ => None,
    }
  }

  pub(super) fn path(self, connections: String) -> String {
    let selected = self.selected().map(|id| id.to_string());
    let decision = self.selected_decision().map(|id| id.to_string());
    with_query(
      connections,
      &[(SELECTED, selected.as_deref()), (DECISION, decision.as_deref())],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    if let Some(id) = params.take(DECISION)? {
      if params.take(SELECTED)?.is_some() {
        return Err(RouteError::InvalidParam {
          param: SELECTED.to_owned(),
          value: id,
        });
      }
      return id.parse().map(Self::decision).map_err(|_| RouteError::InvalidParam {
        param: DECISION.to_owned(),
        value: id,
      });
    }
    let Some(id) = params.take(SELECTED)? else {
      return Ok(Self::default());
    };
    id.parse().map(Self::of).map_err(|_| RouteError::InvalidParam {
      param: SELECTED.to_owned(),
      value: id,
    })
  }
}

/// What Add connection adds: an agent provider (the default), an MCP server, or a decision model
/// (01 XMNUP and 01b z3G09h), which agents cannot run on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NewConnection {
  #[default]
  AgentProvider,
  McpServer,
  DecisionModel,
}

impl NewConnection {
  pub const ALL: [Self; 3] = [Self::AgentProvider, Self::McpServer, Self::DecisionModel];

  /// The kind's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::AgentProvider => "agent_provider",
      Self::McpServer => "mcp_server",
      Self::DecisionModel => "decision_model",
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
