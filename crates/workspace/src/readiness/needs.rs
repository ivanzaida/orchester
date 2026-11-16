//! What each MCP server of the team asks of this computer, and the entry every kind of resource is read into.

use orchester_core::{
  id::{ConnectionId, DecisionConnectionId, ResourceId},
  resource::{Resource, ResourceKind, TeamMcpServer, VersionNumber},
  sharing::{McpNeed, Needs},
};

use super::{facts::Found, standing::Undecided};
use crate::team_config::{TeamConfig, TeamResource};

/// A resource and what it needs, before it is checked against what this computer has.
pub(super) struct Entry {
  pub(super) resource: ResourceId,
  pub(super) kind: ResourceKind,
  pub(super) name: String,
  pub(super) number: VersionNumber,
  pub(super) needs: Result<Needs, Undecided>,
  pub(super) off_here: bool,
  /// What looking at this computer for the needs found (a local project's program files and secret values).
  pub(super) found: Vec<Found>,
}

impl Entry {
  pub(super) fn new(resource: &Resource, number: VersionNumber, needs: Result<Needs, Undecided>) -> Self {
    Self {
      resource: resource.id(),
      kind: resource.kind(),
      name: resource.name().as_str().to_owned(),
      number,
      needs,
      off_here: false,
      found: Vec::new(),
    }
  }

  /// The entry of the team's `resource`, read from its latest version `number`.
  pub(super) fn team(resource: &TeamResource, number: VersionNumber, needs: Result<Needs, Undecided>) -> Self {
    Self {
      resource: resource.id,
      kind: resource.kind,
      name: resource.name.as_str().to_owned(),
      number,
      needs,
      off_here: false,
      found: Vec::new(),
    }
  }
}

/// A team MCP server the project offers, with the version it is read from; `need` is `None` for one whose latest
/// version is not in the team's form, which no profile can be given.
pub(super) struct Server {
  pub(super) entry: Entry,
  pub(super) need: Option<McpNeed>,
}

/// A profile's entry and the connection it runs on: the pick, or its own connection in a local project.
pub(super) struct Profile {
  pub(super) entry: Entry,
  pub(super) pick: Option<ConnectionId>,
}

/// A team reviewer's entry and the connection it runs on: the pick, of the kind its body names.
pub(super) struct Reviewer {
  pub(super) entry: Entry,
  pub(super) pick: Option<ConnectionId>,
  pub(super) decision_pick: Option<DecisionConnectionId>,
}

/// The team's MCP servers, each read from its latest version.
pub(super) fn servers(config: &TeamConfig) -> Vec<Server> {
  let mut servers = Vec::new();
  for resource in config.resources(ResourceKind::McpServer) {
    let (Some(number), Some(body)) = (resource.number(), resource.body::<TeamMcpServer>()) else {
      continue;
    };
    let server = match body {
      Ok(body) => {
        let need = McpNeed::of(resource.id, &body);
        Server {
          entry: Entry::team(resource, number, Ok(Needs::mcp_server(&need))),
          need: Some(need),
        }
      }
      Err(_) => Server {
        entry: Entry::team(resource, number, Err(Undecided::NotTeamForm)),
        need: None,
      },
    };
    servers.push(server);
  }
  servers
}
